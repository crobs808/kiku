import AVFoundation
import CoreAudio
import CoreGraphics
import CoreMedia
import Foundation
import ScreenCaptureKit

private enum HelperError: Error {
    case noDisplay
    case streamStartFailed(String)
}

@available(macOS 13.0, *)
private func canAccessScreenCaptureContent() -> Bool {
    let signal = DispatchSemaphore(value: 0)
    var shareableContent: SCShareableContent?
    var shareableError: Error?

    SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: true) { content, error in
        shareableContent = content
        shareableError = error
        signal.signal()
    }
    signal.wait()

    if shareableError != nil {
        return false
    }
    return shareableContent != nil
}

private func log(_ message: String) {
    let line = "\(message)\n"
    guard let data = line.data(using: .utf8) else { return }
    try? FileHandle.standardError.write(contentsOf: data)
}

private final class BinaryWriter {
    private let handle = FileHandle.standardOutput
    private let lock = NSLock()

    func writeHeader(sampleRateHz: UInt32) throws {
        var header = Data([0x4B, 0x49, 0x4B, 0x55]) // "KIKU"
        var sampleRateLE = sampleRateHz.littleEndian
        withUnsafeBytes(of: &sampleRateLE) { header.append(contentsOf: $0) }

        lock.lock()
        defer { lock.unlock() }
        try handle.write(contentsOf: header)
    }

    func writeSamples(_ samples: [Float]) {
        guard !samples.isEmpty else { return }

        lock.lock()
        defer { lock.unlock() }

        var pcmData = Data(count: samples.count * MemoryLayout<Float>.size)
        pcmData.withUnsafeMutableBytes { destination in
            samples.withUnsafeBytes { source in
                destination.copyMemory(from: source)
            }
        }
        try? handle.write(contentsOf: pcmData)
    }
}

@available(macOS 13.0, *)
private final class SystemAudioCaptureRunner: NSObject, SCStreamOutput, SCStreamDelegate {
    private let sampleRateHz: UInt32 = 48_000
    private let writer = BinaryWriter()
    private let streamQueue = DispatchQueue(label: "kiku.system-audio.stream", qos: .userInitiated)
    private var stream: SCStream?
    private var started = false

    func run() throws -> Never {
        let display = try firstDisplay()

        let filter = SCContentFilter(display: display, excludingWindows: [])
        let config = SCStreamConfiguration()
        config.width = 2
        config.height = 2
        config.queueDepth = 4
        config.minimumFrameInterval = CMTime(value: 1, timescale: 60)
        config.showsCursor = false
        config.capturesAudio = true
        config.sampleRate = Int(sampleRateHz)
        config.channelCount = 2
        config.excludesCurrentProcessAudio = false

        let stream = SCStream(filter: filter, configuration: config, delegate: self)
        do {
            try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: streamQueue)
        } catch {
            throw HelperError.streamStartFailed(error.localizedDescription)
        }

        self.stream = stream

        var startError: NSError?
        let startSignal = DispatchSemaphore(value: 0)
        stream.startCapture { error in
            startError = error as NSError?
            startSignal.signal()
        }
        startSignal.wait()

        if let startError {
            throw HelperError.streamStartFailed(startError.localizedDescription)
        }

        try writer.writeHeader(sampleRateHz: sampleRateHz)
        started = true
        log("READY")
        RunLoop.main.run()
        fatalError("RunLoop.main.run() unexpectedly returned")
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        log("ERROR: stream stopped: \(error.localizedDescription)")
        exit(EXIT_FAILURE)
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
        guard started, outputType == .audio else { return }
        guard let mono = extractMonoFloat32(from: sampleBuffer) else { return }
        writer.writeSamples(mono)
    }

    private func firstDisplay() throws -> SCDisplay {
        let signal = DispatchSemaphore(value: 0)
        var shareableContent: SCShareableContent?
        var shareableError: Error?

        SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: true) {
            content, error in
            shareableContent = content
            shareableError = error
            signal.signal()
        }
        signal.wait()

        if let shareableError {
            throw shareableError
        }

        guard let display = shareableContent?.displays.first else {
            throw HelperError.noDisplay
        }
        return display
    }

    private func extractMonoFloat32(from sampleBuffer: CMSampleBuffer) -> [Float]? {
        guard CMSampleBufferDataIsReady(sampleBuffer) else { return nil }
        guard let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer) else { return nil }
        guard let asbdPointer = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription) else {
            return nil
        }

        let asbd = asbdPointer.pointee
        guard asbd.mFormatID == kAudioFormatLinearPCM else { return nil }

        let frameCount = Int(CMSampleBufferGetNumSamples(sampleBuffer))
        if frameCount <= 0 {
            return nil
        }

        let channelCount = max(1, Int(asbd.mChannelsPerFrame))
        let bytesPerSample = max(1, Int(asbd.mBitsPerChannel) / 8)
        let isFloat = (asbd.mFormatFlags & kAudioFormatFlagIsFloat) != 0
        let audioBufferListSize = MemoryLayout<AudioBufferList>.size + max(0, channelCount - 1) * MemoryLayout<AudioBuffer>.size
        let bufferListPointer = UnsafeMutablePointer<AudioBufferList>.allocate(capacity: 1)
        defer { bufferListPointer.deallocate() }

        var blockBuffer: CMBlockBuffer?
        let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: nil,
            bufferListOut: bufferListPointer,
            bufferListSize: audioBufferListSize,
            blockBufferAllocator: nil,
            blockBufferMemoryAllocator: kCFAllocatorDefault,
            flags: UInt32(kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment),
            blockBufferOut: &blockBuffer
        )
        guard status == noErr else { return nil }

        let buffers = UnsafeMutableAudioBufferListPointer(bufferListPointer)
        guard !buffers.isEmpty else { return nil }

        var mono = Array(repeating: Float(0), count: frameCount)
        if buffers.count == 1 {
            guard let data = buffers[0].mData else { return nil }
            let sampleCount = Int(buffers[0].mDataByteSize) / bytesPerSample

            if isFloat && asbd.mBitsPerChannel == 32 {
                let pointer = data.bindMemory(to: Float.self, capacity: sampleCount)
                if channelCount == 1 {
                    let limit = min(frameCount, sampleCount)
                    for idx in 0..<limit {
                        mono[idx] = pointer[idx]
                    }
                } else {
                    for frame in 0..<frameCount {
                        var sum: Float = 0
                        for channel in 0..<channelCount {
                            let sampleIndex = frame * channelCount + channel
                            if sampleIndex < sampleCount {
                                sum += pointer[sampleIndex]
                            }
                        }
                        mono[frame] = sum / Float(channelCount)
                    }
                }
            } else if !isFloat && asbd.mBitsPerChannel == 16 {
                let pointer = data.bindMemory(to: Int16.self, capacity: sampleCount)
                if channelCount == 1 {
                    let limit = min(frameCount, sampleCount)
                    for idx in 0..<limit {
                        mono[idx] = Float(pointer[idx]) / Float(Int16.max)
                    }
                } else {
                    for frame in 0..<frameCount {
                        var sum: Float = 0
                        for channel in 0..<channelCount {
                            let sampleIndex = frame * channelCount + channel
                            if sampleIndex < sampleCount {
                                sum += Float(pointer[sampleIndex]) / Float(Int16.max)
                            }
                        }
                        mono[frame] = sum / Float(channelCount)
                    }
                }
            } else {
                return nil
            }
            return mono
        }

        let planarChannels = min(channelCount, buffers.count)
        if isFloat && asbd.mBitsPerChannel == 32 {
            for channel in 0..<planarChannels {
                guard let data = buffers[channel].mData else { continue }
                let sampleCount = Int(buffers[channel].mDataByteSize) / bytesPerSample
                let pointer = data.bindMemory(to: Float.self, capacity: sampleCount)
                let limit = min(frameCount, sampleCount)
                for frame in 0..<limit {
                    mono[frame] += pointer[frame]
                }
            }
        } else if !isFloat && asbd.mBitsPerChannel == 16 {
            for channel in 0..<planarChannels {
                guard let data = buffers[channel].mData else { continue }
                let sampleCount = Int(buffers[channel].mDataByteSize) / bytesPerSample
                let pointer = data.bindMemory(to: Int16.self, capacity: sampleCount)
                let limit = min(frameCount, sampleCount)
                for frame in 0..<limit {
                    mono[frame] += Float(pointer[frame]) / Float(Int16.max)
                }
            }
        } else {
            return nil
        }

        let denom = Float(max(1, planarChannels))
        if denom != 1 {
            for frame in 0..<mono.count {
                mono[frame] /= denom
            }
        }
        return mono
    }
}

if #available(macOS 13.0, *) {
    let arguments = Set(CommandLine.arguments.dropFirst())
    if arguments.contains("--permission-status") {
        let granted = canAccessScreenCaptureContent()
        let text = granted ? "granted\n" : "denied\n"
        if let data = text.data(using: .utf8) {
            try? FileHandle.standardOutput.write(contentsOf: data)
        }
        exit(EXIT_SUCCESS)
    }

    if arguments.contains("--request-permission") {
        let granted: Bool
        if canAccessScreenCaptureContent() {
            granted = true
        } else {
            _ = CGRequestScreenCaptureAccess()
            granted = canAccessScreenCaptureContent()
        }
        let text = granted ? "granted\n" : "denied\n"
        if let data = text.data(using: .utf8) {
            try? FileHandle.standardOutput.write(contentsOf: data)
        }
        exit(EXIT_SUCCESS)
    }

    do {
        _ = try SystemAudioCaptureRunner().run()
    } catch HelperError.noDisplay {
        log("ERROR: no display is available for system audio capture")
        exit(EXIT_FAILURE)
    } catch {
        let message = error.localizedDescription.lowercased()
        if message.contains("not authorized")
            || message.contains("permission")
            || message.contains("denied")
            || message.contains("screen capture")
        {
            log("ERROR: screen recording permission denied")
        } else {
            log("ERROR: \(error.localizedDescription)")
        }
        exit(EXIT_FAILURE)
    }
} else {
    log("ERROR: macOS 13 or later is required for system audio capture")
    exit(EXIT_FAILURE)
}

import Foundation

enum KikuCaptureSource: String {
    case microphone
    case systemAudio
}

protocol KikuCapturePlugin {
    func setSourceEnabled(_ source: KikuCaptureSource, enabled: Bool) throws
    func startCapture() throws
    func stopCapture() throws
}

final class StubKikuCapturePlugin: KikuCapturePlugin {
    private var micEnabled = true
    private var systemAudioEnabled = true
    private var running = false

    func setSourceEnabled(_ source: KikuCaptureSource, enabled: Bool) throws {
        switch source {
        case .microphone:
            micEnabled = enabled
        case .systemAudio:
            systemAudioEnabled = enabled
        }
    }

    func startCapture() throws {
        guard !running else { return }
        running = true
    }

    func stopCapture() throws {
        guard running else { return }
        running = false
    }
}

package com.kiku.desktop

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.media.AudioAttributes
import android.media.AudioPlaybackCaptureConfiguration
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.view.WindowManager
import androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.Keep
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicBoolean

@Keep
class MainActivity : TauriActivity() {
  private external fun nativeRegisterActivityInstance()

  private var microphonePermissionRequestInFlight = false
  private var mediaProjectionManager: MediaProjectionManager? = null
  private var mediaProjectionConsentResultCode: Int = Activity.RESULT_CANCELED
  private var mediaProjectionConsentData: Intent? = null
  private var mediaProjection: MediaProjection? = null
  private var systemAudioRecord: AudioRecord? = null
  private var systemAudioCaptureThread: Thread? = null
  private val systemAudioRunning = AtomicBoolean(false)
  private val systemAudioLock = Any()
  private var systemAudioPermissionRequestInFlight = false
  private val backgroundExecutionLock = Any()
  private var backgroundExecutionRefCount = 0
  private var cpuWakeLock: PowerManager.WakeLock? = null
  private var wifiWakeLock: WifiManager.WifiLock? = null
  private val screenAwakeLock = Any()
  private var screenAwakeRefCount = 0

  private val microphonePermissionLauncher =
    registerForActivityResult(ActivityResultContracts.RequestPermission()) { isGranted ->
      microphonePermissionRequestInFlight = false
    }

  private val systemAudioPermissionLauncher = registerForActivityResult(StartActivityForResult()) {
      result ->
    systemAudioPermissionRequestInFlight = false
    if (result.resultCode == Activity.RESULT_OK && result.data != null) {
      synchronized(systemAudioLock) {
        mediaProjectionConsentResultCode = result.resultCode
        mediaProjectionConsentData = result.data
        releaseMediaProjectionLocked()
      }
      return@registerForActivityResult
    }

    synchronized(systemAudioLock) {
      clearSystemAudioConsentLocked()
      stopSystemAudioCaptureLocked()
      releaseMediaProjectionLocked()
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    activeInstance = this
    try {
      nativeRegisterActivityInstance()
    } catch (_: Throwable) {
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      mediaProjectionManager = getSystemService(MediaProjectionManager::class.java)
    }

    requestMicrophonePermission()
  }

  override fun onResume() {
    super.onResume()
    activeInstance = this
  }

  override fun onDestroy() {
    if (activeInstance === this) {
      activeInstance = null
    }
    synchronized(systemAudioLock) {
      stopSystemAudioCaptureLocked()
      releaseMediaProjectionLocked()
      clearSystemAudioConsentLocked()
    }
    synchronized(backgroundExecutionLock) {
      backgroundExecutionRefCount = 0
      releaseBackgroundExecutionLockLocked()
    }
    synchronized(screenAwakeLock) {
      screenAwakeRefCount = 0
    }
    try {
      window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    } catch (_: Throwable) {
    }
    super.onDestroy()
  }

  private fun microphonePermissionStatus(): String {
    if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) ==
      PackageManager.PERMISSION_GRANTED
    ) {
      return "granted"
    }
    return "denied"
  }

  private fun requestMicrophonePermission(): String {
    if (microphonePermissionStatus() == "granted") {
      return "granted"
    }
    if (microphonePermissionRequestInFlight) {
      return "denied"
    }

    microphonePermissionRequestInFlight = true
    runOnUiThread {
      try {
        microphonePermissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
      } catch (_: Throwable) {
        microphonePermissionRequestInFlight = false
      }
    }
    return microphonePermissionStatus()
  }

  @Keep
  fun bridgeSystemAudioPermissionStatus(): String {
    return systemAudioPermissionStatus()
  }

  @Keep
  fun bridgeRequestSystemAudioPermission(): String {
    return requestSystemAudioPermission()
  }

  @Keep
  fun bridgeMicrophonePermissionStatus(): String {
    return microphonePermissionStatus()
  }

  @Keep
  fun bridgeRequestMicrophonePermission(): String {
    return requestMicrophonePermission()
  }

  @Keep
  fun bridgeStartSystemAudioCapture(sampleRateHz: Int): Boolean {
    return startSystemAudioCapture(sampleRateHz)
  }

  @Keep
  fun bridgeStopSystemAudioCapture() {
    stopSystemAudioCapture()
  }

  @Keep
  fun bridgeAcquireBackgroundExecutionLock() {
    acquireBackgroundExecutionLock()
  }

  @Keep
  fun bridgeReleaseBackgroundExecutionLock() {
    releaseBackgroundExecutionLock()
  }

  @Keep
  fun bridgeSetScreenAwakeForDownload(enabled: Boolean) {
    setScreenAwakeForDownload(enabled)
  }

  private fun acquireBackgroundExecutionLock() {
    synchronized(backgroundExecutionLock) {
      backgroundExecutionRefCount += 1
      if (backgroundExecutionRefCount > 1) {
        return
      }

      val powerManager = getSystemService(PowerManager::class.java)
      val nextCpuWakeLock = powerManager?.newWakeLock(
        PowerManager.PARTIAL_WAKE_LOCK,
        "$packageName:model-download"
      )
      nextCpuWakeLock?.setReferenceCounted(false)
      if (nextCpuWakeLock != null && !nextCpuWakeLock.isHeld) {
        try {
          nextCpuWakeLock.acquire(6 * 60 * 60 * 1000L)
        } catch (_: Throwable) {
        }
      }
      cpuWakeLock = nextCpuWakeLock

      val wifiManager = applicationContext.getSystemService(WifiManager::class.java)
      val nextWifiLock =
        wifiManager?.createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "$packageName:model-download")
      nextWifiLock?.setReferenceCounted(false)
      if (nextWifiLock != null && !nextWifiLock.isHeld) {
        try {
          nextWifiLock.acquire()
        } catch (_: Throwable) {
        }
      }
      wifiWakeLock = nextWifiLock
    }
  }

  private fun releaseBackgroundExecutionLock() {
    synchronized(backgroundExecutionLock) {
      if (backgroundExecutionRefCount == 0) {
        return
      }
      backgroundExecutionRefCount -= 1
      if (backgroundExecutionRefCount > 0) {
        return
      }
      releaseBackgroundExecutionLockLocked()
    }
  }

  private fun releaseBackgroundExecutionLockLocked() {
    val currentCpuWakeLock = cpuWakeLock
    if (currentCpuWakeLock != null && currentCpuWakeLock.isHeld) {
      try {
        currentCpuWakeLock.release()
      } catch (_: Throwable) {
      }
    }
    cpuWakeLock = null

    val currentWifiLock = wifiWakeLock
    if (currentWifiLock != null && currentWifiLock.isHeld) {
      try {
        currentWifiLock.release()
      } catch (_: Throwable) {
      }
    }
    wifiWakeLock = null
  }

  private fun setScreenAwakeForDownload(enabled: Boolean) {
    synchronized(screenAwakeLock) {
      if (enabled) {
        screenAwakeRefCount += 1
        if (screenAwakeRefCount > 1) {
          return
        }
      } else {
        if (screenAwakeRefCount == 0) {
          return
        }
        screenAwakeRefCount -= 1
        if (screenAwakeRefCount > 0) {
          return
        }
      }
    }

    runOnUiThread {
      try {
        if (enabled) {
          window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        } else {
          window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
      } catch (_: Throwable) {
      }
    }
  }

  private fun requestSystemAudioPermission(): String {
    if (!supportsSystemAudioPlaybackCapture()) {
      return "unsupported"
    }

    if (systemAudioPermissionStatus() == "granted") {
      return "granted"
    }

    val manager = mediaProjectionManager ?: getSystemService(MediaProjectionManager::class.java)
    if (manager == null) {
      return "denied"
    }

    if (systemAudioPermissionRequestInFlight) {
      return systemAudioPermissionStatus()
    }

    systemAudioPermissionRequestInFlight = true
    runOnUiThread {
      try {
        systemAudioPermissionLauncher.launch(manager.createScreenCaptureIntent())
      } catch (_: Throwable) {
        systemAudioPermissionRequestInFlight = false
      }
    }
    return systemAudioPermissionStatus()
  }

  private fun systemAudioPermissionStatus(): String {
    if (!supportsSystemAudioPlaybackCapture()) {
      return "unsupported"
    }

    synchronized(systemAudioLock) {
      return if (mediaProjection != null || hasSystemAudioConsentLocked()) "granted" else "denied"
    }
  }

  private fun supportsSystemAudioPlaybackCapture(): Boolean {
    return Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q
  }

  private fun startSystemAudioCapture(sampleRateHz: Int): Boolean {
    if (!supportsSystemAudioPlaybackCapture()) {
      return false
    }

    val projectionManager =
      mediaProjectionManager ?: getSystemService(MediaProjectionManager::class.java) ?: return false
    val sampleRate = sampleRateHz.coerceAtLeast(8_000)

    synchronized(systemAudioLock) {
      stopSystemAudioCaptureLocked()

      val projection = ensureMediaProjectionSessionLocked(projectionManager) ?: return false

      val audioFormat = AudioFormat.Builder()
        .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
        .setSampleRate(sampleRate)
        .setChannelMask(AudioFormat.CHANNEL_IN_MONO)
        .build()

      val captureConfig = AudioPlaybackCaptureConfiguration.Builder(projection)
        .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
        .addMatchingUsage(AudioAttributes.USAGE_GAME)
        .build()

      val minBufferBytes = AudioRecord.getMinBufferSize(
        sampleRate,
        AudioFormat.CHANNEL_IN_MONO,
        AudioFormat.ENCODING_PCM_FLOAT
      )
      if (minBufferBytes <= 0) {
        projection.stop()
        return false
      }

      val targetBufferBytes = maxOf(minBufferBytes, sampleRate * 4 / 2)
      val record = try {
        AudioRecord.Builder()
          .setAudioFormat(audioFormat)
          .setBufferSizeInBytes(targetBufferBytes)
          .setAudioPlaybackCaptureConfig(captureConfig)
          .build()
      } catch (error: Throwable) {
        if (error is SecurityException) {
          clearSystemAudioConsentLocked()
          releaseMediaProjectionLocked()
        }
        null
      } ?: return false

      if (record.state != AudioRecord.STATE_INITIALIZED) {
        record.release()
        return false
      }

      try {
        record.startRecording()
      } catch (error: Throwable) {
        record.release()
        if (error is SecurityException) {
          clearSystemAudioConsentLocked()
          releaseMediaProjectionLocked()
        }
        return false
      }

      systemAudioRunning.set(true)
      val captureThread = Thread {
        val buffer = FloatArray(2048)
        while (systemAudioRunning.get()) {
          val read = record.read(buffer, 0, buffer.size, AudioRecord.READ_BLOCKING)
          if (read > 0) {
            nativeOnSystemAudioPcm(
              if (read == buffer.size) buffer.copyOf() else buffer.copyOf(read),
              sampleRate,
              1
            )
            continue
          }

          if (read == AudioRecord.ERROR_DEAD_OBJECT || read == AudioRecord.ERROR_INVALID_OPERATION) {
            break
          }
        }
      }.apply {
        name = "kiku-system-audio-capture"
        isDaemon = true
      }

      mediaProjection = projection
      systemAudioRecord = record
      systemAudioCaptureThread = captureThread
      captureThread.start()
      return true
    }
  }

  private fun stopSystemAudioCapture() {
    synchronized(systemAudioLock) {
      stopSystemAudioCaptureLocked()
    }
  }

  private fun stopSystemAudioCaptureLocked() {
    systemAudioRunning.set(false)

    val captureThread = systemAudioCaptureThread
    if (captureThread != null && captureThread.isAlive) {
      try {
        captureThread.join(400)
      } catch (_: InterruptedException) {
      }
    }
    systemAudioCaptureThread = null

    val record = systemAudioRecord
    if (record != null) {
      try {
        record.stop()
      } catch (_: Throwable) {
      }
      record.release()
      systemAudioRecord = null
    }
  }

  private fun hasSystemAudioConsentLocked(): Boolean {
    return mediaProjectionConsentResultCode == Activity.RESULT_OK && mediaProjectionConsentData != null
  }

  private fun clearSystemAudioConsentLocked() {
    mediaProjectionConsentResultCode = Activity.RESULT_CANCELED
    mediaProjectionConsentData = null
  }

  private fun ensureMediaProjectionSessionLocked(
    projectionManager: MediaProjectionManager
  ): MediaProjection? {
    mediaProjection?.let { return it }
    if (!hasSystemAudioConsentLocked()) {
      return null
    }

    val consentData = mediaProjectionConsentData ?: return null
    val consentCode = mediaProjectionConsentResultCode
    if (consentCode != Activity.RESULT_OK) {
      return null
    }

    val projection = try {
      projectionManager.getMediaProjection(consentCode, consentData)
    } catch (_: Throwable) {
      null
    } ?: run {
      clearSystemAudioConsentLocked()
      releaseMediaProjectionLocked()
      return null
    }

    mediaProjection = projection
    return projection
  }

  private fun releaseMediaProjectionLocked() {
    val projection = mediaProjection
    if (projection != null) {
      try {
        projection.stop()
      } catch (_: Throwable) {
      }
      mediaProjection = null
    }
  }

  companion object {
    @Volatile
    private var activeInstance: MainActivity? = null

    @JvmStatic
    fun systemAudioPermissionStatusNative(): String {
      if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
        return "unsupported"
      }
      return activeInstance?.systemAudioPermissionStatus() ?: "denied"
    }

    @JvmStatic
    fun requestSystemAudioPermissionNative(): String {
      if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
        return "unsupported"
      }
      return activeInstance?.requestSystemAudioPermission() ?: "denied"
    }

    @JvmStatic
    fun microphonePermissionStatusNative(): String {
      return activeInstance?.microphonePermissionStatus() ?: "denied"
    }

    @JvmStatic
    fun requestMicrophonePermissionNative(): String {
      return activeInstance?.requestMicrophonePermission() ?: "denied"
    }

    @JvmStatic
    fun acquireBackgroundExecutionLockNative() {
      activeInstance?.acquireBackgroundExecutionLock()
    }

    @JvmStatic
    fun releaseBackgroundExecutionLockNative() {
      activeInstance?.releaseBackgroundExecutionLock()
    }

    @JvmStatic
    fun startSystemAudioCaptureNative(sampleRateHz: Int): Boolean {
      return activeInstance?.startSystemAudioCapture(sampleRateHz) == true
    }

    @JvmStatic
    fun stopSystemAudioCaptureNative() {
      activeInstance?.stopSystemAudioCapture()
    }

    @Keep
    @JvmStatic
    external fun nativeOnSystemAudioPcm(samples: FloatArray, sampleRateHz: Int, channelCount: Int)
  }
}

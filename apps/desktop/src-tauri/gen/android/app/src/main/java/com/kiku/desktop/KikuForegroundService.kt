package com.kiku.desktop

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

class KikuForegroundService : Service() {
  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val action = intent?.action ?: ACTION_START_OR_UPDATE
    if (action == ACTION_STOP) {
      stopForegroundAndSelf()
      return START_NOT_STICKY
    }

    val requestedTypes = intent?.getIntExtra(EXTRA_FOREGROUND_TYPES, 0) ?: 0
    val foregroundTypes = normalizeForegroundTypes(requestedTypes)
    val statusText = intent?.getStringExtra(EXTRA_STATUS_TEXT)
    startOrUpdateForeground(foregroundTypes, statusText)
    return START_STICKY
  }

  override fun onDestroy() {
    try {
      ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
    } catch (_: Throwable) {
    }
    super.onDestroy()
  }

  private fun startOrUpdateForeground(foregroundTypes: Int, statusText: String?) {
    ensureNotificationChannel()
    val notification = buildNotification(statusText)
    ServiceCompat.startForeground(this, NOTIFICATION_ID, notification, foregroundTypes)
  }

  private fun stopForegroundAndSelf() {
    try {
      ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
    } catch (_: Throwable) {
    }
    stopSelf()
  }

  private fun ensureNotificationChannel() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
      return
    }

    val manager = getSystemService(NotificationManager::class.java) ?: return
    if (manager.getNotificationChannel(CHANNEL_ID) != null) {
      return
    }

    val channel = NotificationChannel(
      CHANNEL_ID,
      "Kiku Background Activity",
      NotificationManager.IMPORTANCE_LOW
    ).apply {
      description = "Keeps active transcription and downloads running in the background."
      setShowBadge(false)
      lockscreenVisibility = android.app.Notification.VISIBILITY_PRIVATE
    }
    manager.createNotificationChannel(channel)
  }

  private fun buildNotification(statusText: String?): android.app.Notification {
    val openAppIntent = Intent(this, MainActivity::class.java).apply {
      flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
    }
    val pendingFlags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
      PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
    } else {
      PendingIntent.FLAG_UPDATE_CURRENT
    }
    val openAppPendingIntent = PendingIntent.getActivity(this, 0, openAppIntent, pendingFlags)

    return NotificationCompat.Builder(this, CHANNEL_ID)
      .setSmallIcon(R.mipmap.ic_launcher)
      .setContentTitle("Kiku is active")
      .setContentText(statusText ?: "Background activity in progress.")
      .setOngoing(true)
      .setOnlyAlertOnce(true)
      .setCategory(NotificationCompat.CATEGORY_SERVICE)
      .setContentIntent(openAppPendingIntent)
      .build()
  }

  private fun normalizeForegroundTypes(types: Int): Int {
    return if (types == 0) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else types
  }

  companion object {
    private const val ACTION_START_OR_UPDATE = "com.kiku.desktop.action.START_OR_UPDATE_KEEPALIVE"
    private const val ACTION_STOP = "com.kiku.desktop.action.STOP_KEEPALIVE"
    private const val EXTRA_FOREGROUND_TYPES = "foreground_types"
    private const val EXTRA_STATUS_TEXT = "status_text"
    private const val CHANNEL_ID = "kiku_background_activity"
    private const val NOTIFICATION_ID = 44_001

    fun startOrUpdate(context: Context, foregroundTypes: Int, statusText: String) {
      val intent = Intent(context, KikuForegroundService::class.java).apply {
        action = ACTION_START_OR_UPDATE
        putExtra(EXTRA_FOREGROUND_TYPES, foregroundTypes)
        putExtra(EXTRA_STATUS_TEXT, statusText)
      }
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        context.startForegroundService(intent)
      } else {
        context.startService(intent)
      }
    }

    fun stop(context: Context) {
      context.stopService(Intent(context, KikuForegroundService::class.java))
    }
  }
}

package io.juspay.sudoquery.internal

import android.app.ActivityManager
import android.app.Application
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.util.Log
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner

internal interface Clock {
    /** Wall-clock time, for event timestamps, file ages and sessions. */
    fun nowMillis(): Long

    /** Monotonic time that ignores clock changes, for retry backoff. */
    fun elapsedMillis(): Long
}

internal object AndroidClock : Clock {
    override fun nowMillis(): Long = System.currentTimeMillis()

    override fun elapsedMillis(): Long = android.os.SystemClock.elapsedRealtime()
}

internal interface Logger {
    fun debug(message: String)

    fun warn(message: String, error: Throwable? = null)

    fun error(message: String, error: Throwable? = null)
}

internal class AndroidLogger(private val debugEnabled: Boolean) : Logger {
    override fun debug(message: String) {
        if (debugEnabled) Log.d(TAG, message)
    }

    override fun warn(message: String, error: Throwable?) {
        Log.w(TAG, message, error)
    }

    override fun error(message: String, error: Throwable?) {
        Log.e(TAG, message, error)
    }

    private companion object {
        const val TAG = "SudoQuery"
    }
}

/**
 * True in the app's main process. The SDK runs only there: a second process writing the same
 * queue directory would interleave and re-upload batches.
 */
internal fun isMainProcess(context: Context): Boolean {
    val mainProcess = context.applicationInfo.processName ?: context.packageName
    val current = currentProcessName(context) ?: return true
    return current == mainProcess
}

private fun currentProcessName(context: Context): String? {
    if (Build.VERSION.SDK_INT >= 28) return Application.getProcessName()
    val activityManager = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager ?: return null
    val pid = Process.myPid()
    return activityManager.runningAppProcesses?.firstOrNull { it.pid == pid }?.processName
}

/** Reports the app moving between foreground and background, via [ProcessLifecycleOwner]. */
internal class AppLifecycle(
    private val onForeground: () -> Unit,
    private val onBackground: () -> Unit,
) : DefaultLifecycleObserver {

    override fun onStart(owner: LifecycleOwner) = onForeground()

    override fun onStop(owner: LifecycleOwner) = onBackground()

    /** Observers must be added on the main thread. Without lifecycle events the flush timer still runs. */
    fun register(logger: Logger) {
        Handler(Looper.getMainLooper()).post {
            try {
                ProcessLifecycleOwner.get().lifecycle.addObserver(this)
            } catch (e: Throwable) {
                logger.warn("App lifecycle unavailable; events are sent by the flush timer only", e)
            }
        }
    }
}

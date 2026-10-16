package dev.skene.spike.text

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity

/**
 * An empty activity the Compose comparison attaches its view to, and the high-contrast probe
 * draws in; it keeps the screen on and shows over the lock screen so a screenshot sees it.
 */
class SpikeActivity : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    setShowWhenLocked(true)
    setTurnScreenOn(true)
    window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
  }
}

package dev.sawitulm.palmannotate.rust

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var webViewRef: WebView? = null

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webViewRef = webView
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // TauriActivity disables wry's built-in back handling (handleBackNavigation
    // = false), so by default any back gesture finishes the activity and closes
    // the app. Bridge back to in-app navigation: __paBack() returns "back" when
    // it consumed the press, or "exit" when already on Home.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val webView = webViewRef
        if (webView == null) {
          finishFromBack()
          return
        }
        webView.evaluateJavascript(
          "(window.__paBack && window.__paBack()) || 'exit'"
        ) { result ->
          val handled = result != null && result.contains("back")
          if (!handled) {
            finishFromBack()
          }
        }
      }

      private fun finishFromBack() {
        isEnabled = false
        onBackPressedDispatcher.onBackPressed()
        isEnabled = true
      }
    })
  }
}

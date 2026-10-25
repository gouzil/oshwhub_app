package expo.modules.oshwhubcore

import android.content.Context
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class OshwhubCoreModule : Module() {
  companion object { init { System.loadLibrary("oshwhub_ffi") } }
  private external fun nativeDispatch(input: String): String
  private external fun nativeInitialize(directory: String)
  private val preferences
    get() = requireNotNull(appContext.reactContext).getSharedPreferences("oshwhub-settings", Context.MODE_PRIVATE)

  override fun definition() = ModuleDefinition {
    Name("OshwhubCore")
    Function("getThemeColor") { preferences.getString("themeColor", null) }
    Function("setThemeColor") { value: String ->
      check(preferences.edit().putString("themeColor", value).commit()) { "Unable to save theme color" }
    }
    OnCreate {
      appContext.reactContext?.cacheDir?.absolutePath?.let { nativeInitialize(it) }
    }
    AsyncFunction("dispatch") { input: String -> nativeDispatch(input) }
  }
}

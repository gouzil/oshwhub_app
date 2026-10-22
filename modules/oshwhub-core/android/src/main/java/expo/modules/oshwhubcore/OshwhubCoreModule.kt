package expo.modules.oshwhubcore

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class OshwhubCoreModule : Module() {
  companion object { init { System.loadLibrary("oshwhub_ffi") } }
  private external fun nativeDispatch(input: String): String
  private external fun nativeInitialize(directory: String)

  override fun definition() = ModuleDefinition {
    Name("OshwhubCore")
    OnCreate {
      appContext.reactContext?.cacheDir?.absolutePath?.let { nativeInitialize(it) }
    }
    AsyncFunction("dispatch") { input: String -> nativeDispatch(input) }
  }
}

import ExpoModulesCore
import Foundation

public class OshwhubCoreModule: Module {
  public func definition() -> ModuleDefinition {
    Name("OshwhubCore")
    OnCreate {
      let cache = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first?.path ?? ""
      cache.withCString { oshwhub_initialize($0) }
    }
    AsyncFunction("dispatch") { (input: String) -> String in
      input.withCString { pointer in
        guard let response = oshwhub_execute(pointer) else {
          return "{\"ok\":false,\"error\":{\"code\":\"internal\",\"message\":\"Rust核心没有响应\",\"requestId\":\"ffi\"}}"
        }
        defer { oshwhub_free(response) }
        return String(cString: response)
      }
    }
    View(LiquidTabBar.self) {
      Events("onSelect")
      Prop("items") { (view, items: [NavigationItem]) in
        view.setItems(items)
      }
      Prop("selectedIndex") { (view, index: Int) in
        view.select(index)
      }
    }
  }
}

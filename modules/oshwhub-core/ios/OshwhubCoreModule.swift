import ExpoModulesCore
import Foundation
import UIKit

public class OshwhubCoreModule: Module {
  public func definition() -> ModuleDefinition {
    Name("OshwhubCore")
    Function("getThemeColor") {
      UserDefaults.standard.string(forKey: "themeColor")
    }
    Function("setThemeColor") { (value: String) in
      UserDefaults.standard.set(value, forKey: "themeColor")
    }
    Function("getThemeMode") {
      UserDefaults.standard.string(forKey: "themeMode")
    }
    Function("setThemeMode") { (value: String) in
      UserDefaults.standard.set(value, forKey: "themeMode")
    }
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
      Prop("tintColor") { (view, color: UIColor) in
        view.setTintColor(color)
      }
      Prop("colorScheme") { (view, scheme: String) in
        view.setColorScheme(scheme)
      }
    }
  }
}

import ExpoModulesCore
import UIKit

struct NavigationItem: Record {
  @Field var label: String = ""
  @Field var symbol: String = ""
}

// Let UIKit own the lens, symbol sizing, touch tracking and accessibility appearance.
final class LiquidTabBar: ExpoView, UITabBarDelegate {
  let onSelect = EventDispatcher()
  private let bar = UITabBar()
  private var selectedIndex = 0

  required init(appContext: AppContext? = nil) {
    super.init(appContext: appContext)
    backgroundColor = .clear
    bar.delegate = self
    bar.tintColor = UIColor(red: 44 / 255, green: 101 / 255, blue: 232 / 255, alpha: 1)
    addSubview(bar)
  }

  override func layoutSubviews() {
    super.layoutSubviews()
    bar.frame = bounds
  }

  func setItems(_ items: [NavigationItem]) {
    bar.setItems(items.enumerated().map { index, item in
      UITabBarItem(title: item.label, image: UIImage(systemName: item.symbol), tag: index)
    }, animated: false)
    select(selectedIndex)
  }

  func select(_ index: Int) {
    selectedIndex = index
    guard let items = bar.items, items.indices.contains(index) else { return }
    bar.selectedItem = items[index]
  }

  func setTintColor(_ color: UIColor) {
    bar.tintColor = color
  }

  func setColorScheme(_ scheme: String) {
    bar.overrideUserInterfaceStyle = scheme == "dark" ? .dark : .light
  }

  func tabBar(_ tabBar: UITabBar, didSelect item: UITabBarItem) {
    selectedIndex = item.tag
    onSelect(["index": item.tag])
  }
}

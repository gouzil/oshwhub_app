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
    bar.tintColor = UIColor(red: 0, green: 0.51, blue: 0.37, alpha: 1)
    bar.overrideUserInterfaceStyle = .light
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

  func tabBar(_ tabBar: UITabBar, didSelect item: UITabBarItem) {
    selectedIndex = item.tag
    onSelect(["index": item.tag])
  }
}

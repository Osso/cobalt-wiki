## DOM Compatibility

Backwards compatibility with Wikidot is an important goal of the Wikijump project. In order to allow imported data from Wikidot to be usable in Wikijump, the project implements a transition mechanism called "layout" for each site or page to choose its HTML structure, either conforming to Wikidot (legacy) layout or to the new Wikijump layout.

However, there are some places where we have determined it would be better not to maintain DOM compatibility. Here is a brief list of them.

UI where themes and customization are not available:
  * Login / logout page
  * User settings page
  * Admin panel
  * User profile page

UI where existing themes should be adapted to:
  * Page editor
  * Page options

## Legacy tabview interaction

Legacy-layout tabviews remain interactive without a component import or client-side widget initialization. A layout-level delegated handler activates only a tabview's matching direct-child panel, retaining the server-selected initial panel until activation. Activation updates the selected tab's `aria-selected` and `tabindex`, and the corresponding panel's visibility and ARIA state.

Tab controls support pointer activation plus keyboard navigation: Arrow keys, Home, and End move focus among sibling tabs; Enter and Space activate the focused tab. Nested or unrelated tabviews must not be affected.

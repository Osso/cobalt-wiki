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

## FTML tabview compatibility

In legacy/Wikidot layout, compiled `wj-tabs` markup receives the source theme's YUI-style tab presentation: distinct bordered, padded controls; a source-blue selected control; wrapped long labels; and bordered, padded panels. This is a layout/theme compatibility concern only and does not alter page content.

### Interaction

Compiled `wj-tabs` tabviews use layout-level delegated handlers rather than custom-element registration. Activation shows only the matching direct-child panel, retaining the server-selected initial panel until activation. It updates sibling tabs' `aria-selected` and `tabindex` attributes and panels' `hidden` state.

Tab controls support pointer activation plus keyboard navigation: Arrow keys, Home, and End move focus among sibling tabs; Enter and Space activate the focused tab. Nested or unrelated tabviews must not be affected.

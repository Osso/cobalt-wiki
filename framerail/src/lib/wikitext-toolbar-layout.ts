// Toolbar layout in the source editor's control order, drawn with Lucide icons.
// Controls whose source icon is a text label keep that text.

import {
  Bold,
  BookOpen,
  Brackets,
  Code,
  CodeXml,
  Columns2,
  FileSymlink,
  Heading1,
  Heading2,
  Heading3,
  Heading4,
  Heading5,
  Heading6,
  Image,
  Italic,
  Link,
  List,
  ListIndentDecrease,
  ListIndentIncrease,
  ListOrdered,
  ListTree,
  Minus,
  PanelLeft,
  PanelRight,
  Parentheses,
  Quote,
  Radical,
  SquareDashed,
  Strikethrough,
  Subscript,
  Superscript,
  Table,
  TableOfContents,
  Underline,
  Variable,
  WandSparkles,
  type IconNode
} from "lucide"

/**
 * One button: an icon, or static trusted text markup when the source icon
 * is text.
 */
export type ToolbarControl = {
  id: string
  label: string
  icon?: IconNode
  text?: string
}

/**
 * A control, a control with an attached wizard, or a control with a
 * submenu.
 */
export type ToolbarItem =
  | ToolbarControl
  | { main: ToolbarControl; wizard: ToolbarControl }
  | { main: ToolbarControl; menuLabel: string; menu: ToolbarControl[] }

const headingIcons = [Heading1, Heading2, Heading3, Heading4, Heading5, Heading6]

const heading = (level: number): ToolbarControl => ({
  id: `heading${level}`,
  label: `heading level ${level}`,
  icon: headingIcons[level - 1]
})

const withWizard = (main: ToolbarControl, id: string, label: string) => ({
  main,
  wizard: { id, label, icon: WandSparkles }
})

export const toolbarRows: ToolbarItem[][] = [
  [
    {
      main: heading(1),
      menuLabel: "Other heading levels",
      menu: [2, 3, 4, 5, 6].map(heading)
    },
    { id: "bold", label: "bold", icon: Bold },
    { id: "italic", label: "italic", icon: Italic },
    { id: "underline", label: "underline", icon: Underline },
    { id: "strikethrough", label: "strikethrough", icon: Strikethrough },
    { id: "teletype", label: "teletype", text: '<span class="mono">tt</span>' },
    { id: "quote", label: "quote", icon: Quote },
    { id: "superscript", label: "superscript", icon: Superscript },
    { id: "subscript", label: "subscript", icon: Subscript },
    { id: "raw", label: "raw", text: '<span class="mono">R</span>' }
  ],
  [
    { id: "hr", label: "horizontal rule", icon: Minus },
    { id: "div", label: "div", icon: SquareDashed },
    {
      main: { id: "clearFloat", label: "clear float", icon: Columns2 },
      menuLabel: "Clear float direction",
      menu: [
        { id: "clearFloatLeft", label: "clear float left", icon: PanelLeft },
        { id: "clearFloatRight", label: "clear float right", icon: PanelRight }
      ]
    },
    { id: "tableWizard", label: "table wizard", icon: Table },
    { id: "toc", label: "table of contents", icon: TableOfContents }
  ],
  [
    withWizard(
      { id: "code", label: "code", icon: Code },
      "codeWizard",
      "code block wizard"
    ),
    withWizard({ id: "uri", label: "url", icon: Link }, "uriWizard", "URL link wizard"),
    withWizard(
      { id: "pageLink", label: "page link", icon: FileSymlink },
      "pageLinkWizard",
      "page link wizard"
    ),
    withWizard(
      { id: "image", label: "image", icon: Image },
      "imageWizard",
      "image wizard"
    ),
    { id: "html", label: "HTML", icon: CodeXml }
  ],
  [
    { id: "numberedList", label: "numbered list", icon: ListOrdered },
    { id: "bulletedList", label: "bulleted list", icon: List },
    { id: "increaseListIndent", label: "increase indent", icon: ListIndentIncrease },
    { id: "decreaseListIndent", label: "decrease indent", icon: ListIndentDecrease },
    { id: "definitionList", label: "definition list", icon: ListTree },
    { id: "footnote", label: "footnote", text: "fn<sup>1</sup>" }
  ],
  [
    { id: "math", label: "math", icon: Radical },
    { id: "inlineMath", label: "inline math", icon: Variable },
    { id: "erefWizard", label: "equation reference", icon: Parentheses },
    { id: "bibliography", label: "bibliography", icon: BookOpen },
    { id: "bibliographycitation", label: "bibcite", icon: Brackets }
  ]
]

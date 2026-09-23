import postcssHtml from "postcss-html"
import postcssScss from "postcss-scss"

/** @type {import("stylelint").Config} */
const config = {
  extends: ["stylelint-config-recess-order"],
  plugins: ["stylelint-scss"],
  ignoreFiles: ["**/node_modules/**", "./build/**", "./svelte-kit/**", "./package/**"],
  reportNeedlessDisables: true,
  reportInvalidScopeDisables: true,
  defaultSeverity: "warning",
  rules: {
    "color-no-invalid-hex": true,
    "function-linear-gradient-no-nonstandard-direction": true,
    "length-zero-no-unit": true,
    "shorthand-property-no-redundant-values": true,
    "comment-no-empty": true,
    "scss/selector-no-redundant-nesting-selector": true
  },
  overrides: [
    {
      files: ["**/*.scss"],
      customSyntax: postcssScss
    },
    {
      files: ["**/*.svelte"],
      customSyntax: postcssHtml
    }
  ]
}

export default config

import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "target/**",
      "src-tauri/**",
      "crates/**",
      "coverage/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.ts"],
    languageOptions: { globals: { ...globals.browser } },
    rules: {
      "no-restricted-properties": [
        "error",
        {
          property: "innerHTML",
          message: "Utiliser textContent / createElement (données non fiables).",
        },
        {
          property: "outerHTML",
          message: "Utiliser textContent / createElement (données non fiables).",
        },
      ],
    },
  },
  {
    files: ["*.js", "*.ts"],
    languageOptions: { globals: { ...globals.node } },
  },
);

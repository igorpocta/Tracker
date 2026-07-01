/**
 * ESLint flat config (Phase 18C — Item 25).
 *
 * Scope: TypeScript + React only — Rust is covered by `cargo fmt` / `cargo
 * clippy` in the pre-commit. Sensible rules with warnings (not errors) for
 * `no-explicit-any` and `no-unused-vars` so existing code passes today
 * without a giant cleanup; CI can be tightened later by flipping these to
 * errors.
 */
import js from "@eslint/js";
import tsParser from "@typescript-eslint/parser";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import reactPlugin from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";

export default [
  // 1) Files we never want to lint. Order matters in flat config: ignores in
  //    the FIRST config object are global.
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "src-tauri/target/**",
      "src-tauri/icons/**",
      "scripts/**",
      // Browser extension: standalone WebExtension bundle with its own
      // runtime globals (chrome, window, document…) — not part of the app's
      // TS lint surface.
      "browser-extension/**",
      "**/*.config.js",
      "**/*.config.ts",
      // Build/test entry that imports config helpers.
      "vite.config.ts",
      "vitest.config.ts",
      "eslint.config.js",
    ],
  },

  // 2) Recommended JS rules — keeps things sane for any plain-JS file.
  js.configs.recommended,

  // 3) TypeScript + React project rules.
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        ecmaFeatures: { jsx: true },
      },
      globals: {
        window: "readonly",
        document: "readonly",
        navigator: "readonly",
        console: "readonly",
        setTimeout: "readonly",
        setInterval: "readonly",
        clearTimeout: "readonly",
        clearInterval: "readonly",
        requestAnimationFrame: "readonly",
        cancelAnimationFrame: "readonly",
        fetch: "readonly",
        URL: "readonly",
        URLSearchParams: "readonly",
        Blob: "readonly",
        File: "readonly",
        FormData: "readonly",
        FileReader: "readonly",
        localStorage: "readonly",
        sessionStorage: "readonly",
        HTMLElement: "readonly",
        HTMLInputElement: "readonly",
        HTMLButtonElement: "readonly",
        HTMLDivElement: "readonly",
        HTMLTextAreaElement: "readonly",
        HTMLSelectElement: "readonly",
        Element: "readonly",
        Event: "readonly",
        KeyboardEvent: "readonly",
        MouseEvent: "readonly",
        Node: "readonly",
        crypto: "readonly",
        AbortController: "readonly",
        process: "readonly",
        global: "readonly",
        // Vitest globals (test files).
        describe: "readonly",
        it: "readonly",
        test: "readonly",
        expect: "readonly",
        beforeEach: "readonly",
        afterEach: "readonly",
        beforeAll: "readonly",
        afterAll: "readonly",
        vi: "readonly",
      },
    },
    plugins: {
      "@typescript-eslint": tsPlugin,
      react: reactPlugin,
      "react-hooks": reactHooks,
    },
    settings: {
      react: { version: "detect" },
    },
    rules: {
      // -- TypeScript --
      // `no-explicit-any` is a warning — we have a few intentional `any`s
      // in test mocks. Tighten later if we drive them all to `unknown`.
      "@typescript-eslint/no-explicit-any": "warn",
      // Allow `_`-prefixed unused vars (idiomatic for destructure).
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // Use the TS-aware versions; turn the JS one off to avoid false
      // positives on types/interfaces/enums.
      "no-unused-vars": "off",
      "no-undef": "off", // TS handles this.

      // -- React --
      // React 17+ JSX transform — no need to import React in scope.
      "react/react-in-jsx-scope": "off",
      // We rely on TS for prop typing, not PropTypes.
      "react/prop-types": "off",
      // Allow `<a>` without `target="_blank"` rel — opener plugin handles it.
      "react/no-unescaped-entities": "off",

      // -- React Hooks --
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",

      // -- Style --
      "prefer-const": "warn",
      "no-empty": ["warn", { allowEmptyCatch: true }],
      // We use `===` everywhere already; make the warning visible if anyone
      // sneaks a `==` in.
      eqeqeq: ["warn", "smart"],
    },
  },
];

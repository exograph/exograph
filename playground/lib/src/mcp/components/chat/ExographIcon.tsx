import { default as LogoNoText } from "../../../../public/logo-no-text.svg";

export function ExographIcon({ className }: { className?: string }) {
  // Vite treats imports as strings, but Webpack (in Docusaurus configuration) treats them as React components.
  // So, we use `vite-plugin-svgr` in Vite vite to convert SVGs to React components, but we need to cast them to `any` to avoid TypeScript errors.
  const Icon = LogoNoText as any;

  return <Icon className={className} alt="Exograph" />;
}

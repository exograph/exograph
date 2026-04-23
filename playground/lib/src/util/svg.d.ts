declare module '*.svg' {
  import type { ComponentType, SVGProps } from 'react';
  const content: ComponentType<SVGProps<SVGSVGElement> & { alt?: string; title?: string }>;
  export default content;
}
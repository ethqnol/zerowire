//! @zerowire/unplugin: Ahead-of-Time build plugin for Vite, Rolldown, esbuild, and Rollup.

import { createUnplugin } from "unplugin";
import { compileWireCodec } from "@zerowire/napi";

export interface ZeroWirePluginOptions {
  /** Target file extensions to transform (default: ['.ts', '.tsx']) */
  include?: string[];
}

export const unplugin = createUnplugin<ZeroWirePluginOptions | undefined>((options = {}) => {
  const includeExts = options?.include ?? [".ts", ".tsx"];

  return {
    name: "zerowire-plugin",
    transformInclude(id) {
      return includeExts.some((ext) => id.endsWith(ext));
    },
    transform(code, id) {
      if (!code.includes("compileWireCodec") && !code.includes("compileCodec")) {
        return null;
      }

      try {
        const result = compileWireCodec(code, undefined);

        // Replace compileWireCodec<T>() macro calls with compiled codec instantiation
        const macroRegex = /(?:export\s+const\s+(\w+)\s*=\s*)?compile(?:Wire)?Codec<(\w+)>\(\);?/g;

        const transformed = code.replace(macroRegex, (_match, varName, _typeName) => {
          const exportVar = varName ? `export const ${varName} = ${result.targetType}Codec;` : "";
          return `${result.jsCode}\n${exportVar}`;
        });

        return {
          code: transformed,
          map: null,
        };
      } catch (err) {
        console.error(`[@zerowire/unplugin] Failed to transform ${id}:`, err);
        return null;
      }
    },
  };
});

export const vite = unplugin.vite;
export const rollup = unplugin.rollup;
export const esbuild = unplugin.esbuild;
export default unplugin;

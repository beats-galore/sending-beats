// Local oxlint rules, loaded through the jsPlugin bridge.
//
// `naming-convention` reproduces the old eslint config's
// `@typescript-eslint/naming-convention` (`{ selector: ['typeLike'], format:
// ['PascalCase'] }`). We can't use @typescript-eslint's own rule because it
// calls `getParserServices()` for the `@typescript-eslint/parser` TS<->ESTree
// node maps, which oxlint's bridge doesn't provide (oxlint uses its own parser
// + tsgolint). This is a pure-AST equivalent that needs no type services.

const isPascalCase = (name) => /^[A-Z][a-zA-Z0-9]*$/.test(name);

/** @type {import('oxlint').Rule} */
const namingConvention = {
  meta: {
    type: 'suggestion',
    messages: {
      pascal: 'Type name `{{name}}` must be PascalCase.',
    },
  },
  create(context) {
    const check = (node) => {
      const name = node.id?.name;
      if (typeof name === 'string' && !isPascalCase(name)) {
        context.report({ node: node.id, messageId: 'pascal', data: { name } });
      }
    };
    return {
      TSInterfaceDeclaration: check,
      TSTypeAliasDeclaration: check,
      TSEnumDeclaration: check,
      ClassDeclaration: check,
    };
  },
};

export default {
  meta: { name: 'local-rules' },
  rules: {
    'naming-convention': namingConvention,
  },
};

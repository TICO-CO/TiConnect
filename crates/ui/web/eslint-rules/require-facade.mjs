import fileSystem from 'node:fs';
import path from 'node:path';

const extensionPattern = /\.(?:[cm]?[jt]s)$/;
const facadeNames = new Set(['index', 'main']);
const facadeFiles = ['index.ts', 'index.mts', 'index.cts', 'index.js', 'index.mjs', 'index.cjs'];

const isRelative = (specifier) => specifier.startsWith('./') || specifier.startsWith('../');

const directoryHasFacade = (directory) =>
    facadeFiles.some((name) => fileSystem.existsSync(path.join(directory, name)));

const isFacadeTarget = (targetPath) => {
    const baseName = path.basename(targetPath).replace(extensionPattern, '');
    if (facadeNames.has(baseName)) return true;
    try {
        return fileSystem.statSync(targetPath).isDirectory() && directoryHasFacade(targetPath);
    } catch {
        return false;
    }
};

const sourceOf = (node) => node.source?.value;

export default {
    meta: {
        type: 'problem',
        docs: {
            description: 'Require relative imports from outside a folder to target its facade.',
        },
        schema: [],
        messages: {
            nonFacadeImport:
                'Import the nearest index facade instead of reaching into "{{source}}".',
        },
    },
    create(context) {
        const filename = context.filename ?? context.getFilename();
        const sourceDirectory = path.dirname(filename);

        const checkImport = (node) => {
            const source = sourceOf(node);
            if (!source || !isRelative(source)) return;

            const targetPath = path.resolve(sourceDirectory, source);
            if (path.dirname(targetPath) === sourceDirectory) return;
            if (isFacadeTarget(targetPath)) return;

            context.report({ node, messageId: 'nonFacadeImport', data: { source } });
        };

        return {
            ImportDeclaration: checkImport,
            ExportAllDeclaration: checkImport,
            ExportNamedDeclaration: checkImport,
        };
    },
};

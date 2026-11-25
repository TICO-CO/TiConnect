import prettier from 'eslint-config-prettier';
import simpleImportSort from 'eslint-plugin-simple-import-sort';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';

import noComments from './eslint-rules/no-comments.mjs';
import noThrowStatements from './eslint-rules/no-throw-statements.mjs';
import noTryStatements from './eslint-rules/no-try-statements.mjs';
import requireFacade from './eslint-rules/require-facade.mjs';
import svelteConfig from './svelte.config.js';

const localPlugin = {
    rules: {
        'no-comments': noComments,
        'no-throw-statements': noThrowStatements,
        'no-try-statements': noTryStatements,
        'require-facade': requireFacade,
    },
};

const styleRules = {
    '@ticonnect/no-comments': 'error',
    '@ticonnect/no-throw-statements': 'error',
    '@ticonnect/no-try-statements': 'error',
    '@ticonnect/require-facade': 'error',
    '@typescript-eslint/no-explicit-any': 'error',
    '@typescript-eslint/no-non-null-assertion': 'error',
    '@typescript-eslint/switch-exhaustiveness-check': 'error',
    'simple-import-sort/exports': 'error',
    'simple-import-sort/imports': 'error',
};

const localPlugins = {
    '@ticonnect': localPlugin,
    '@typescript-eslint': tseslint.plugin,
    'simple-import-sort': simpleImportSort,
};

export default tseslint.config(
    {
        ignores: ['node_modules/**', 'dist/**', 'bun.lock'],
    },
    ...svelte.configs['flat/recommended'],
    ...svelte.configs['flat/prettier'],
    {
        files: ['**/*.{ts,mts,cts}'],
        languageOptions: {
            parser: tseslint.parser,
            parserOptions: {
                projectService: true,
                tsconfigRootDir: import.meta.dirname,
                ecmaVersion: 'latest',
                sourceType: 'module',
            },
        },
        plugins: localPlugins,
        rules: styleRules,
    },
    {
        files: ['**/*.svelte'],
        languageOptions: {
            parserOptions: {
                parser: tseslint.parser,
                extraFileExtensions: ['.svelte'],
                projectService: true,
                tsconfigRootDir: import.meta.dirname,
                svelteConfig,
            },
        },
        plugins: localPlugins,
        rules: styleRules,
    },
    {
        files: ['src/lib/result.ts'],
        rules: {
            '@ticonnect/no-try-statements': 'off',
        },
    },
    {
        files: ['src/lib/assert.ts'],
        rules: {
            '@ticonnect/no-throw-statements': 'off',
        },
    },
    {
        files: ['src/vite-env.d.ts'],
        rules: {
            '@ticonnect/no-comments': 'off',
        },
    },
    prettier,
);

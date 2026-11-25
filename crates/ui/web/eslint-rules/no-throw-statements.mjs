export default {
    meta: {
        type: 'problem',
        docs: {
            description: 'Disallow throw statements outside the sanctioned assert module.',
        },
        schema: [],
        messages: {
            unexpectedThrow: 'Use Result instead of throwing. Only src/lib/assert.ts may throw.',
        },
    },
    create(context) {
        return {
            ThrowStatement(node) {
                context.report({ node, messageId: 'unexpectedThrow' });
            },
        };
    },
};

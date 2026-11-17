export default {
    meta: {
        type: 'problem',
        docs: {
            description: 'Disallow try statements outside the sanctioned Result module.',
        },
        schema: [],
        messages: {
            unexpectedTry:
                'Use attempt or attemptAsync instead. Only src/lib/result.ts may use try.',
        },
    },
    create(context) {
        return {
            TryStatement(node) {
                context.report({ node, messageId: 'unexpectedTry' });
            },
        };
    },
};

export default {
    meta: {
        type: 'suggestion',
        docs: {
            description: 'Disallow comments in TypeScript and JavaScript sources.',
        },
        schema: [],
        messages: {
            unexpectedComment: 'Code comments are not allowed. Use descriptive names instead.',
        },
    },
    create(context) {
        const sourceCode = context.sourceCode ?? context.getSourceCode();
        return {
            Program() {
                for (const comment of sourceCode.getAllComments()) {
                    context.report({ loc: comment.loc, messageId: 'unexpectedComment' });
                }
            },
        };
    },
};

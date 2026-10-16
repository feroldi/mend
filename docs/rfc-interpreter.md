# RFC: Mend compiler/interpreter

The compiler/interpreter has the following parts:

1. Driver: Guides the compilation and execution process, from CLI down to running a Mend program.
2. Lexer: Converts the input source code to a stream of tokens.
3. Parser: Parses the stream of tokens into an AST (uses bumpalo to store the AST, just like sophia does).
4. Codegen: Translates the AST into the Mend byte code.
5. VM: The virtual machine that executes the Mend byte code.

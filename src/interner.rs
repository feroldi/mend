use std::collections::HashMap;

use bumpalo::Bump;

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub(crate) struct Symbol(usize);

#[derive(Default)]
pub(crate) struct StringInterner {
    interned_strs: HashMap<&'static str, Symbol>,
    indexed_strs: Vec<&'static str>,
    buffer: Bump,
}

impl StringInterner {
    pub(crate) fn get_or_intern(&mut self, string: &str) -> Symbol {
        if let Some(&symbol) = self.interned_strs.get(string) {
            return symbol;
        }

        let symbol = Symbol(self.indexed_strs.len());

        let buffed_string = {
            let allocated_str = self.buffer.alloc_str(string);

            unsafe {
                // SAFETY: The memory allocated by `buffer` will live for as long as the
                // interner does, and symbols are only resolved through the interner.
                &*(allocated_str as *const str)
            }
        };

        self.interned_strs.insert(buffed_string, symbol);
        self.indexed_strs.push(buffed_string);

        symbol
    }

    pub(crate) fn resolve(&self, symbol: Symbol) -> &'static str {
        self.indexed_strs[symbol.0]
    }
}

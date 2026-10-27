use super::*;

impl Parser {
    // PHP parses clone($object)->property as clone ($object->property), but
    // clone($object,) uses the function-call grammar and needs parentheses
    // around the entire call before a subsequent property access.
    pub(super) fn clone_call_arguments(&self) -> bool {
        if !self.is("(") {
            return false;
        }
        let rest = &self.tokens[self.pos + 1..];
        if rest
            .first()
            .is_some_and(|t| matches!(&t.kind, Kind::Symbol(s) if s == ")" || s == "..."))
            || (rest
                .first()
                .is_some_and(|t| matches!(t.kind, Kind::Word(_)))
                && rest
                    .get(1)
                    .is_some_and(|t| matches!(&t.kind, Kind::Symbol(s) if s == ":")))
        {
            return true;
        }
        let mut depth = 0;
        for token in rest {
            if let Kind::Symbol(symbol) = &token.kind {
                match symbol.as_str() {
                    "," if depth == 0 => return true,
                    ")" if depth == 0 => return false,
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    _ => {}
                }
            }
        }
        false
    }
}

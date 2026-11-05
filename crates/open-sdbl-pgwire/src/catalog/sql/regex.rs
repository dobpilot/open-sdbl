//! Budgeted regular expression subset, shared with LIKE translation.
use super::{Budget, ErrorFields};
#[derive(Debug)]
enum Node {
    Empty,
    Char(char),
    Any,
    Class(bool, Vec<(char, char)>),
    Start,
    End,
    Seq(Vec<Node>),
    Or(Vec<Node>),
    Repeat(Box<Node>, usize, Option<usize>),
}
struct Parser {
    chars: Vec<char>,
    i: usize,
    depth: usize,
}
impl Parser {
    fn expression(&mut self) -> Result<Node, ErrorFields> {
        self.depth += 1;
        if self.depth > 64 {
            return Err(ErrorFields::error("54001", "regex nesting exceeds 64"));
        }
        let mut branches = vec![];
        loop {
            let mut sequence = vec![];
            while self.i < self.chars.len() && !matches!(self.chars[self.i], ')' | '|') {
                sequence.push(self.atom()?);
            }
            branches.push(if sequence.is_empty() {
                Node::Empty
            } else {
                Node::Seq(sequence)
            });
            if self.chars.get(self.i) != Some(&'|') {
                break;
            }
            self.i += 1;
        }
        self.depth -= 1;
        Ok(if branches.len() == 1 {
            branches.remove(0)
        } else {
            Node::Or(branches)
        })
    }
    fn atom(&mut self) -> Result<Node, ErrorFields> {
        let c = self.chars[self.i];
        self.i += 1;
        let mut node = match c {
            '(' => {
                let e = self.expression()?;
                self.need(')')?;
                e
            }
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '[' => {
                let negated = self.chars.get(self.i) == Some(&'^');
                if negated {
                    self.i += 1;
                }
                let mut ranges = vec![];
                while self.i < self.chars.len() && self.chars[self.i] != ']' {
                    let a = self.character()?;
                    if a == '[' && self.chars.get(self.i) == Some(&':') {
                        return Err(ErrorFields::error(
                            "0A000",
                            "POSIX character classes are unsupported",
                        ));
                    }
                    if a == '\\' {
                        return Err(ErrorFields::error(
                            "0A000",
                            "escaped bracket expressions are unsupported",
                        ));
                    }
                    if self.chars.get(self.i) == Some(&'-')
                        && self.chars.get(self.i + 1) != Some(&']')
                    {
                        self.i += 1;
                        let b = self.character()?;
                        if a > b {
                            return Err(ErrorFields::error("2201B", "invalid bracket range"));
                        }
                        ranges.push((a, b));
                    } else {
                        ranges.push((a, a));
                    }
                }
                self.need(']')?;
                Node::Class(negated, ranges)
            }
            '\\' => {
                let c = self.character()?;
                if c.is_alphanumeric() {
                    return Err(ErrorFields::error("0A000", "unsupported regex escape"));
                }
                Node::Char(c)
            }
            '*' | '+' | '?' | '{' | '}' => {
                return Err(ErrorFields::error("2201B", "invalid regex quantifier"));
            }
            c => Node::Char(c),
        };
        if let Some(c) = self.chars.get(self.i).copied() {
            let bounds = match c {
                '*' => Some((0, None)),
                '+' => Some((1, None)),
                '?' => Some((0, Some(1))),
                '{' => {
                    self.i += 1;
                    let min = self.number()?;
                    let max = if self.chars.get(self.i) == Some(&',') {
                        self.i += 1;
                        if self.chars.get(self.i) == Some(&'}') {
                            None
                        } else {
                            Some(self.number()?)
                        }
                    } else {
                        Some(min)
                    };
                    self.need('}')?;
                    if max.is_some_and(|n| n < min) || min > 10000 || max.is_some_and(|n| n > 10000)
                    {
                        return Err(ErrorFields::error(
                            "54001",
                            "regex repetition exceeds bound",
                        ));
                    }
                    node = Node::Repeat(Box::new(node), min, max);
                    return Ok(node);
                }
                _ => None,
            };
            if let Some((min, max)) = bounds {
                self.i += 1;
                node = Node::Repeat(Box::new(node), min, max);
            }
        }
        Ok(node)
    }
    fn character(&mut self) -> Result<char, ErrorFields> {
        let c = self
            .chars
            .get(self.i)
            .copied()
            .ok_or_else(|| ErrorFields::error("2201B", "unterminated regex"))?;
        self.i += 1;
        Ok(c)
    }
    fn need(&mut self, c: char) -> Result<(), ErrorFields> {
        if self.character()? == c {
            Ok(())
        } else {
            Err(ErrorFields::error("2201B", "invalid regex"))
        }
    }
    fn number(&mut self) -> Result<usize, ErrorFields> {
        let mut n = String::new();
        while self.chars.get(self.i).is_some_and(char::is_ascii_digit) {
            n.push(self.character()?);
        }
        n.parse()
            .map_err(|_| ErrorFields::error("2201B", "invalid regex bound"))
    }
}
fn positions(
    node: &Node,
    text: &[char],
    start: usize,
    budget: &mut Budget,
) -> Result<Vec<usize>, ErrorFields> {
    budget.step()?;
    let single = |yes| if yes { vec![start] } else { vec![] };
    let result = match node {
        Node::Empty => vec![start],
        Node::Start => single(start == 0),
        Node::End => single(start == text.len()),
        Node::Char(c) => {
            if text.get(start) == Some(c) {
                vec![start + 1]
            } else {
                vec![]
            }
        }
        Node::Any => {
            if start < text.len() {
                vec![start + 1]
            } else {
                vec![]
            }
        }
        Node::Class(negated, ranges) => {
            if text
                .get(start)
                .is_some_and(|c| ranges.iter().any(|(a, b)| a <= c && c <= b) != *negated)
            {
                vec![start + 1]
            } else {
                vec![]
            }
        }
        Node::Or(nodes) => {
            let mut out = vec![];
            for node in nodes {
                out.extend(positions(node, text, start, budget)?);
                budget.rows(out.len())?;
            }
            out.sort_unstable();
            out.dedup();
            out
        }
        Node::Seq(nodes) => {
            let mut out = vec![start];
            for node in nodes {
                let mut next = vec![];
                for pos in out {
                    next.extend(positions(node, text, pos, budget)?);
                    budget.rows(next.len())?;
                }
                next.sort_unstable();
                next.dedup();
                out = next;
                if out.is_empty() {
                    break;
                }
            }
            out
        }
        Node::Repeat(node, min, max) => {
            let mut frontier = vec![start];
            let mut out = if *min == 0 { vec![start] } else { vec![] };
            let max = max.unwrap_or(text.len().saturating_add(*min).saturating_add(1));
            for count in 1..=max {
                let mut next = vec![];
                for pos in frontier {
                    for end in positions(node, text, pos, budget)? {
                        if end != pos || count <= *min {
                            next.push(end);
                        }
                    }
                    budget.rows(next.len())?;
                }
                next.sort_unstable();
                next.dedup();
                if count >= *min {
                    out.extend(&next);
                    budget.rows(out.len())?;
                }
                if next.is_empty() {
                    break;
                }
                frontier = next;
            }
            out.sort_unstable();
            out.dedup();
            out
        }
    };
    budget.bytes = budget
        .bytes
        .checked_sub(result.len().saturating_mul(std::mem::size_of::<usize>()))
        .ok_or_else(|| ErrorFields::error("54001", "regex allocation budget exceeded"))?;
    Ok(result)
}
pub(in crate::catalog) fn matches(
    pattern: &str,
    text: &str,
    insensitive: bool,
    budget: &mut Budget,
) -> Result<bool, ErrorFields> {
    let (pattern, text) = if insensitive {
        (pattern.to_lowercase(), text.to_lowercase())
    } else {
        (pattern.into(), text.into())
    };
    let mut parser = Parser {
        chars: pattern.chars().collect(),
        i: 0,
        depth: 0,
    };
    let node = parser.expression()?;
    if parser.i != parser.chars.len() {
        return Err(ErrorFields::error("2201B", "unbalanced regex"));
    }
    let text = text.chars().collect::<Vec<_>>();
    for start in 0..=text.len() {
        if !positions(&node, &text, start, budget)?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn like(
    pattern: &str,
    text: &str,
    insensitive: bool,
    budget: &mut Budget,
) -> Result<bool, ErrorFields> {
    let mut regex = String::from("^");
    let mut escape = false;
    for c in pattern.chars() {
        if !escape && c == '\\' {
            escape = true;
            continue;
        }
        if !escape && c == '%' {
            regex.push_str(".*");
        } else if !escape && c == '_' {
            regex.push('.');
        } else {
            if ".^$[](){}*+?|\\".contains(c) {
                regex.push('\\');
            }
            regex.push(c);
        }
        escape = false;
    }
    if escape {
        return Err(ErrorFields::error("22025", "LIKE pattern ends with escape"));
    }
    regex.push('$');
    matches(&regex, text, insensitive, budget)
}

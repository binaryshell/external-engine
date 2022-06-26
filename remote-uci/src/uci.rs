use std::{
    collections::HashMap,
    fmt,
    hash::{Hash, Hasher},
    num::{NonZeroU32, ParseIntError},
    time::Duration,
};

use memchr::{memchr2, memchr2_iter};
use shakmaty::{
    fen::{Fen, ParseFenError},
    uci::{ParseUciError, Uci},
};
use thiserror::Error;

#[derive(Clone, Debug, Eq)]
pub struct UciOptionName(String);

impl PartialEq for UciOptionName {
    fn eq(&self, other: &UciOptionName) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Hash for UciOptionName {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        for byte in self.0.bytes() {
            hasher.write_u8(byte.to_ascii_lowercase());
        }
        hasher.write_u8(0xff);
    }
}

impl fmt::Display for UciOptionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UciOptionValue(String);

impl fmt::Display for UciOptionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UciOption {
    Check { default: bool },
    Spin { default: i64, min: i64, max: i64 },
    Combo { default: String, var: Vec<String> },
    Button,
    String { default: String },
}

impl fmt::Display for UciOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UciOption::Check { default } => write!(f, "type check default {default}"),
            UciOption::Spin { default, min, max } => {
                write!(f, "type spin default {default} min {min} max {max}")
            }
            UciOption::Combo { default, var } => {
                write!(f, "type combo default {default}")?;
                for v in var {
                    write!(f, " var {v}")?;
                }
                Ok(())
            }
            UciOption::Button => f.write_str("type button"),
            UciOption::String { default } => write!(f, "type string default {default}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UciIn {
    Uci,
    Isready,
    Setoption {
        name: UciOptionName,
        value: Option<UciOptionValue>,
    },
    Ucinewgame,
    Position {
        fen: Option<Fen>,
        moves: Vec<Uci>,
    },
    Go {
        searchmoves: Option<Vec<Uci>>,
        ponder: bool,
        wtime: Option<Duration>,
        btime: Option<Duration>,
        winc: Option<Duration>,
        binc: Option<Duration>,
        movestogo: Option<u32>,
        depth: Option<u32>,
        nodes: Option<u64>,
        mate: Option<u32>,
        movetime: Option<Duration>,
        infinite: bool,
    },
    Stop,
    Ponderhit,
}

impl UciIn {
    pub fn from_line(s: &str) -> Result<Option<UciIn>, ProtocolError> {
        Parser::new(s)?.parse_in()
    }
}

impl fmt::Display for UciIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UciIn::Uci => f.write_str("uci"),
            UciIn::Isready => f.write_str("isready"),
            UciIn::Setoption { name, value } => {
                write!(f, "setoption name {name}")?;
                if let Some(value) = value {
                    write!(f, " value {value}")?;
                }
                Ok(())
            }
            UciIn::Ucinewgame => f.write_str("ucinewgame"),
            UciIn::Position { fen, moves } => {
                match fen {
                    Some(fen) => write!(f, "position fen {fen}")?,
                    None => f.write_str("position startpos")?,
                }
                if !moves.is_empty() {
                    f.write_str(" moves ")?;
                    for m in moves {
                        write!(f, " {}", m)?;
                    }
                }
                Ok(())
            }
            UciIn::Go {
                searchmoves,
                ponder,
                wtime,
                btime,
                winc,
                binc,
                movestogo,
                depth,
                nodes,
                mate,
                movetime,
                infinite,
            } => {
                f.write_str("go")?;
                if let Some(searchmoves) = searchmoves {
                    f.write_str(" searchmoves")?;
                    for m in searchmoves {
                        write!(f, " {}", m)?;
                    }
                }
                if *ponder {
                    f.write_str(" ponder")?;
                }
                if let Some(wtime) = wtime {
                    write!(f, " wtime {}", wtime.as_millis())?;
                }
                if let Some(btime) = btime {
                    write!(f, " btime {}", btime.as_millis())?;
                }
                if let Some(winc) = winc {
                    write!(f, " winc {}", winc.as_millis())?;
                }
                if let Some(binc) = binc {
                    write!(f, " binc {}", binc.as_millis())?;
                }
                if let Some(movestogo) = movestogo {
                    write!(f, " movestogo {movestogo}")?;
                }
                if let Some(depth) = depth {
                    write!(f, " depth {depth}")?;
                }
                if let Some(nodes) = nodes {
                    write!(f, " nodes {nodes}")?;
                }
                if let Some(mate) = mate {
                    write!(f, " mate {mate}")?;
                }
                if let Some(movetime) = movetime {
                    write!(f, " movetime {}", movetime.as_millis())?;
                }
                if *infinite {
                    f.write_str(" infinite")?;
                }
                Ok(())
            }
            UciIn::Stop => f.write_str("stop"),
            UciIn::Ponderhit => f.write_str("ponderhit"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eval {
    Cp(i64),
    Mate(i32),
}

impl fmt::Display for Eval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Eval::Cp(cp) => write!(f, "cp {cp}"),
            Eval::Mate(mate) => write!(f, "mate {mate}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    eval: Eval,
    lowerbound: bool,
    upperbound: bool,
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.eval.fmt(f)?;
        if self.lowerbound {
            f.write_str(" lowerbound")?;
        }
        if self.upperbound {
            f.write_str(" upperbound")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UciOut {
    IdName(String),
    IdAuthor(String),
    Uciok,
    Readyok,
    Bestmove {
        m: Option<Uci>,
        ponder: Option<Uci>,
    },
    Info {
        multipv: Option<NonZeroU32>,
        depth: Option<u32>,
        seldepth: Option<u32>,
        time: Option<Duration>,
        nodes: Option<u64>,
        pv: Option<Vec<Uci>>,
        score: Option<Score>,
        currmove: Option<Uci>,
        currmovenumber: Option<u32>,
        hashfull: Option<u32>,
        nps: Option<u64>,
        tbhits: Option<u64>,
        sbhits: Option<u64>,
        cpuload: Option<u32>,
        refutation: HashMap<Uci, Vec<Uci>>,
        currline: HashMap<u32, Vec<Uci>>,
        string: Option<String>,
    },
    Option {
        name: UciOptionName,
        option: UciOption,
    },
}

impl UciOut {
    pub fn from_line(s: &str) -> Result<Option<UciOut>, ProtocolError> {
        Parser::new(s)?.parse_out()
    }
}

impl fmt::Display for UciOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UciOut::IdName(name) => write!(f, "id name {name}"),
            UciOut::IdAuthor(author) => write!(f, "id author {author}"),
            UciOut::Uciok => f.write_str("uciok"),
            UciOut::Readyok => f.write_str("readyok"),
            UciOut::Bestmove { m, ponder } => {
                match m {
                    Some(m) => write!(f, "bestmove {m}")?,
                    None => f.write_str("bestmove (none)")?,
                }
                if let Some(ponder) = ponder {
                    write!(f, " ponder {ponder}")?;
                }
                Ok(())
            }
            UciOut::Info {
                multipv,
                depth,
                seldepth,
                time,
                nodes,
                pv,
                score,
                currmove,
                currmovenumber,
                hashfull,
                nps,
                tbhits,
                sbhits,
                cpuload,
                refutation,
                currline,
                string,
            } => {
                f.write_str("info")?;
                if let Some(multipv) = multipv {
                    write!(f, " multipv {multipv}")?;
                }
                if let Some(depth) = depth {
                    write!(f, " depth {depth}")?;
                }
                if let Some(seldepth) = seldepth {
                    write!(f, " seldepth {seldepth}")?;
                }
                if let Some(time) = time {
                    write!(f, " time {}", time.as_millis())?;
                }
                if let Some(nodes) = nodes {
                    write!(f, " nodes {nodes}")?;
                }
                if let Some(pv) = pv {
                    f.write_str(" pv")?;
                    for m in pv {
                        write!(f, " {m}")?;
                    }
                }
                if let Some(score) = score {
                    write!(f, " score {score}")?;
                }
                if let Some(currmove) = currmove {
                    write!(f, " currmove {currmove}")?;
                }
                if let Some(currmovenumber) = currmovenumber {
                    write!(f, " currmovenumber {currmovenumber}")?;
                }
                if let Some(hashfull) = hashfull {
                    write!(f, " hashfull {hashfull}")?;
                }
                if let Some(nps) = nps {
                    write!(f, " nps {nps}")?;
                }
                if let Some(tbhits) = tbhits {
                    write!(f, " tbhits {tbhits}")?;
                }
                if let Some(sbhits) = sbhits {
                    write!(f, " sbhits {sbhits}")?;
                }
                if let Some(cpuload) = cpuload {
                    write!(f, " cpuload {cpuload}")?;
                }
                for (refuted, refuted_by) in refutation {
                    write!(f, " refutation {refuted}")?;
                    for m in refuted_by {
                        write!(f, " {m}")?;
                    }
                }
                for (cpunr, currline) in currline {
                    write!(f, " currline {cpunr}")?;
                    for m in currline {
                        write!(f, " {m}")?;
                    }
                }
                if let Some(string) = string {
                    write!(f, " string {string}")?;
                }
                Ok(())
            }
            UciOut::Option { name, option } => write!(f, "option name {name} {option}"),
        }
    }
}

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("unexpected token")]
    UnexpectedToken,
    #[error("unexpected line break in uci command")]
    UnexpectedLineBreak,
    #[error("expected end of line")]
    ExpectedEndOfLine,
    #[error("unexpected end of line")]
    UnexpectedEndOfLine,
    #[error("invalid fen: {0}")]
    InvalidFen(#[from] ParseFenError),
    #[error("invalid move: {0}")]
    InvalidMove(#[from] ParseUciError),
    #[error("invalid integer: {0}")]
    InvalidInteger(#[from] ParseIntError),
}

struct Parser<'a> {
    s: &'a str,
}

impl<'a> Iterator for Parser<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let (head, tail) = read(self.s);
        self.s = tail;
        head
    }
}

impl<'a> Parser<'a> {
    pub fn new(s: &str) -> Result<Parser<'_>, ProtocolError> {
        match memchr2(b'\r', b'\n', s.as_bytes()) {
            Some(_) => Err(ProtocolError::UnexpectedLineBreak),
            None => Ok(Parser { s }),
        }
    }

    fn peek(&self) -> Option<&str> {
        let (head, _) = read(self.s);
        head
    }

    fn until<P>(&mut self, pred: P) -> Option<&str>
    where
        P: FnMut(&'a str) -> bool,
    {
        let (head, tail) = read_until(self.s, pred);
        self.s = tail;
        head
    }

    fn end(&self) -> Result<(), ProtocolError> {
        match self.peek() {
            Some(_) => Err(ProtocolError::ExpectedEndOfLine),
            None => Ok(()),
        }
    }

    fn parse_setoption(&mut self) -> Result<UciIn, ProtocolError> {
        Ok(match self.next() {
            Some("name") => UciIn::Setoption {
                name: UciOptionName(
                    self.until(|t| t == "value")
                        .ok_or(ProtocolError::UnexpectedEndOfLine)?
                        .to_owned(),
                ),
                value: match self.next() {
                    Some("value") => Some(UciOptionValue(
                        self.until(|_| false)
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .to_owned(),
                    )),
                    Some(_) => unreachable!(),
                    None => None,
                },
            },
            Some(_) => return Err(ProtocolError::UnexpectedToken),
            None => return Err(ProtocolError::UnexpectedEndOfLine),
        })
    }

    fn parse_position(&mut self) -> Result<UciIn, ProtocolError> {
        Ok(UciIn::Position {
            fen: match self.until(|t| t == "moves") {
                Some("startpos") => None,
                Some(fen) => Some(fen.parse()?),
                None => return Err(ProtocolError::UnexpectedEndOfLine),
            },
            moves: match self.next() {
                Some("moves") => self
                    .map(|m| m.parse())
                    .collect::<Result<_, ParseUciError>>()?,
                Some(_) => unreachable!(),
                None => Vec::new(),
            },
        })
    }

    fn parse_millis(&mut self) -> Result<Duration, ProtocolError> {
        Ok(Duration::from_millis(
            self.next()
                .ok_or(ProtocolError::UnexpectedEndOfLine)?
                .parse()?,
        ))
    }

    fn parse_moves(&mut self) -> Vec<Uci> {
        let mut moves = Vec::new();
        while let Some(m) = self.peek() {
            match m.parse() {
                Ok(uci) => {
                    self.next();
                    moves.push(uci);
                }
                Err(_) => break,
            }
        }
        moves
    }

    fn parse_go(&mut self) -> Result<UciIn, ProtocolError> {
        let mut searchmoves = None;
        let mut ponder = false;
        let mut wtime = None;
        let mut btime = None;
        let mut winc = None;
        let mut binc = None;
        let mut movestogo = None;
        let mut depth = None;
        let mut nodes = None;
        let mut mate = None;
        let mut movetime = None;
        let mut infinite = false;
        loop {
            match self.next() {
                Some("ponder") => ponder = true,
                Some("infinite") => infinite = true,
                Some("movestogo") => {
                    movestogo = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("depth") => {
                    depth = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("nodes") => {
                    nodes = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("mate") => {
                    mate = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("movetime") => movetime = Some(self.parse_millis()?),
                Some("wtime") => wtime = Some(self.parse_millis()?),
                Some("btime") => btime = Some(self.parse_millis()?),
                Some("winc") => winc = Some(self.parse_millis()?),
                Some("binc") => binc = Some(self.parse_millis()?),
                Some("searchmoves") => searchmoves = Some(self.parse_moves()),
                Some(_) => return Err(ProtocolError::UnexpectedToken),
                None => break,
            }
        }
        Ok(UciIn::Go {
            searchmoves,
            ponder,
            wtime,
            btime,
            winc,
            binc,
            movestogo,
            depth,
            nodes,
            mate,
            movetime,
            infinite,
        })
    }

    fn parse_in(&mut self) -> Result<Option<UciIn>, ProtocolError> {
        Ok(Some(match self.next() {
            Some("uci") => {
                self.end()?;
                UciIn::Uci
            }
            Some("isready") => {
                self.end()?;
                UciIn::Isready
            }
            Some("ucinewgame") => {
                self.end()?;
                UciIn::Ucinewgame
            }
            Some("stop") => {
                self.end()?;
                UciIn::Stop
            }
            Some("ponderhit") => {
                self.end()?;
                UciIn::Ponderhit
            }
            Some("setoption") => self.parse_setoption()?,
            Some("position") => self.parse_position()?,
            Some("go") => self.parse_go()?,
            Some(_) => return Err(ProtocolError::UnexpectedToken),
            None => return Ok(None),
        }))
    }

    fn parse_option(&mut self) -> Result<UciOut, ProtocolError> {
        let name = match self.next() {
            Some("name") => UciOptionName(
                self.until(|t| t == "type")
                    .ok_or(ProtocolError::UnexpectedEndOfLine)?
                    .to_owned(),
            ),
            Some(_) => return Err(ProtocolError::UnexpectedToken),
            None => return Err(ProtocolError::UnexpectedEndOfLine),
        };
        self.next(); // type
        let option = match self.next() {
            Some("check") => UciOption::Check {
                default: match self.next() {
                    Some("default") => match self.next() {
                        Some("true") => true,
                        Some("false") => false,
                        Some(_) => return Err(ProtocolError::UnexpectedToken),
                        None => return Err(ProtocolError::UnexpectedEndOfLine),
                    },
                    Some(_) => return Err(ProtocolError::UnexpectedToken),
                    None => return Err(ProtocolError::UnexpectedEndOfLine),
                },
            },
            Some("spin") => {
                let mut default = None;
                let mut min = None;
                let mut max = None;
                loop {
                    match self.next() {
                        Some("default") => {
                            default = Some(
                                self.next()
                                    .ok_or(ProtocolError::UnexpectedEndOfLine)?
                                    .parse()?,
                            )
                        }
                        Some("min") => {
                            min = Some(
                                self.next()
                                    .ok_or(ProtocolError::UnexpectedEndOfLine)?
                                    .parse()?,
                            )
                        }
                        Some("max") => {
                            max = Some(
                                self.next()
                                    .ok_or(ProtocolError::UnexpectedEndOfLine)?
                                    .parse()?,
                            )
                        }
                        Some(_) => return Err(ProtocolError::UnexpectedToken),
                        None => break,
                    }
                }
                UciOption::Spin {
                    default: default.ok_or(ProtocolError::UnexpectedEndOfLine)?,
                    min: min.ok_or(ProtocolError::UnexpectedEndOfLine)?,
                    max: max.ok_or(ProtocolError::UnexpectedEndOfLine)?,
                }
            }
            Some("combo") => {
                let mut default = None;
                let mut var = Vec::new();
                let eot = |t| t == "default" || t == "var";
                loop {
                    match self.next() {
                        Some("default") => {
                            default = Some(
                                self.until(eot)
                                    .ok_or(ProtocolError::UnexpectedEndOfLine)?
                                    .to_owned(),
                            )
                        }
                        Some("var") => var.push(
                            self.until(eot)
                                .ok_or(ProtocolError::UnexpectedEndOfLine)?
                                .to_owned(),
                        ),
                        Some(_) => return Err(ProtocolError::UnexpectedToken),
                        None => break,
                    }
                }
                UciOption::Combo {
                    default: default.ok_or(ProtocolError::UnexpectedEndOfLine)?,
                    var,
                }
            }
            Some("button") => {
                self.end()?;
                UciOption::Button
            }
            Some("string") => UciOption::String {
                default: match self.next() {
                    Some("default") => self.until(|_| false).unwrap_or_default().to_owned(),
                    Some(_) => return Err(ProtocolError::UnexpectedToken),
                    None => return Err(ProtocolError::UnexpectedEndOfLine),
                },
            },
            Some(_) => return Err(ProtocolError::UnexpectedToken),
            None => return Err(ProtocolError::UnexpectedEndOfLine),
        };
        Ok(UciOut::Option { name, option })
    }

    fn parse_bestmove(&mut self) -> Result<UciOut, ProtocolError> {
        Ok(UciOut::Bestmove {
            m: match self.next() {
                Some("(none)") | None => None,
                Some(m) => Some(m.parse()?),
            },
            ponder: match self.next() {
                Some("ponder") => match self.next() {
                    Some("(none)") | None => None,
                    Some(m) => Some(m.parse()?),
                },
                Some(_) => return Err(ProtocolError::UnexpectedToken),
                None => None,
            },
        })
    }

    fn parse_score(&mut self) -> Result<Score, ProtocolError> {
        let eval = match self.next() {
            Some("cp") => Eval::Cp(
                self.next()
                    .ok_or(ProtocolError::UnexpectedEndOfLine)?
                    .parse()?,
            ),
            Some("mate") => Eval::Mate(
                self.next()
                    .ok_or(ProtocolError::UnexpectedEndOfLine)?
                    .parse()?,
            ),
            Some(_) => return Err(ProtocolError::UnexpectedToken),
            None => return Err(ProtocolError::UnexpectedEndOfLine),
        };
        let mut lowerbound = false;
        let mut upperbound = false;
        while let Some(token) = self.peek() {
            match token {
                "lowerbound" => {
                    self.next();
                    lowerbound = true;
                }
                "upperbound" => {
                    self.next();
                    upperbound = true;
                }
                _ => break,
            }
        }
        Ok(Score {
            eval,
            lowerbound,
            upperbound,
        })
    }

    fn parse_info(&mut self) -> Result<UciOut, ProtocolError> {
        let mut multipv = None;
        let mut depth = None;
        let mut seldepth = None;
        let mut time = None;
        let mut nodes = None;
        let mut pv = None;
        let mut score = None;
        let mut currmove = None;
        let mut currmovenumber = None;
        let mut hashfull = None;
        let mut nps = None;
        let mut tbhits = None;
        let mut sbhits = None;
        let mut cpuload = None;
        let mut refutation = HashMap::new();
        let mut currline = HashMap::new();
        let mut string = None;
        loop {
            match self.next() {
                Some("multipv") => {
                    multipv = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("depth") => {
                    depth = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("seldepth") => {
                    seldepth = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("time") => {
                    time = Some(Duration::from_millis(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    ))
                }
                Some("nodes") => {
                    nodes = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("pv") => pv = Some(self.parse_moves()),
                Some("score") => score = Some(self.parse_score()?),
                Some("currmove") => {
                    currmove = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("currmovenumber") => {
                    currmovenumber = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("hashfull") => {
                    hashfull = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("nps") => {
                    nps = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("tbhits") => {
                    tbhits = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("sbhits") => {
                    sbhits = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("cpuload") => {
                    cpuload = Some(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                    )
                }
                Some("refutation") => {
                    refutation.insert(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                        self.parse_moves(),
                    );
                }
                Some("currline") => {
                    currline.insert(
                        self.next()
                            .ok_or(ProtocolError::UnexpectedEndOfLine)?
                            .parse()?,
                        self.parse_moves(),
                    );
                }
                Some("string") => {
                    string = Some(self.until(|_| false).unwrap_or_default().to_owned())
                }
                Some(_) => return Err(ProtocolError::UnexpectedToken),
                None => break,
            }
        }
        Ok(UciOut::Info {
            multipv,
            depth,
            seldepth,
            time,
            nodes,
            pv,
            score,
            currmove,
            currmovenumber,
            hashfull,
            nps,
            tbhits,
            sbhits,
            cpuload,
            refutation,
            currline,
            string,
        })
    }

    fn parse_out(&mut self) -> Result<Option<UciOut>, ProtocolError> {
        Ok(Some(match self.next() {
            Some("id") => match self.next() {
                Some("name") => UciOut::IdName(
                    self.until(|_| false)
                        .ok_or(ProtocolError::UnexpectedEndOfLine)?
                        .to_owned(),
                ),
                Some("author") => UciOut::IdAuthor(
                    self.until(|_| false)
                        .ok_or(ProtocolError::UnexpectedEndOfLine)?
                        .to_owned(),
                ),
                Some(_) => return Err(ProtocolError::UnexpectedToken),
                None => return Err(ProtocolError::UnexpectedEndOfLine),
            },
            Some("uciok") => UciOut::Uciok,
            Some("readyok") => UciOut::Readyok,
            Some("bestmove") => self.parse_bestmove()?,
            Some("info") => self.parse_info()?,
            Some("option") => self.parse_option()?,
            Some(_) => return Err(ProtocolError::UnexpectedToken),
            None => return Ok(None),
        }))
    }
}

fn is_separator(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn read(s: &str) -> (Option<&str>, &str) {
    let s = s.trim_start_matches(is_separator);
    if s.is_empty() {
        (None, s)
    } else {
        let (head, tail) = s.split_at(memchr2(b' ', b'\t', s.as_bytes()).unwrap_or(s.len()));
        (Some(head), tail)
    }
}

fn read_until<'a, P>(s: &'a str, mut pred: P) -> (Option<&'a str>, &'a str)
where
    P: FnMut(&'a str) -> bool,
{
    let s = s.trim_start_matches(is_separator);
    if s.is_empty() {
        (None, "")
    } else {
        for end in memchr2_iter(b' ', b'\t', s.as_bytes()) {
            let (head, tail) = s.split_at(end);
            if let (Some(next_token), _) = read(tail) {
                if pred(next_token) {
                    return (Some(head), tail);
                }
            }
        }
        (Some(s), "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read() {
        assert_eq!(read(""), (None, ""));
        assert_eq!(read(" abc\t def g"), (Some("abc"), "\t def g"));
        assert_eq!(read("  end"), (Some("end"), ""));
    }

    #[test]
    fn test_read_until() {
        assert_eq!(
            read_until("abc def value foo", |t| t == "value"),
            (Some("abc def"), " value foo")
        );
        assert_eq!(
            read_until("abc def valuefoo", |t| t == "value"),
            (Some("abc def valuefoo"), "")
        );
        assert_eq!(
            read_until("value abc", |t| t == "value"),
            (Some("value abc"), "")
        );
    }

    #[test]
    fn test_setoption() -> Result<(), ProtocolError> {
        assert_eq!(
            UciIn::from_line("setoption name Skill Level value 10")?,
            Some(UciIn::Setoption {
                name: UciOptionName("skill level".to_owned()),
                value: Some(UciOptionValue("10".to_owned()))
            })
        );

        assert_eq!(
            UciIn::from_line("setoption name Clear Hash")?,
            Some(UciIn::Setoption {
                name: UciOptionName("clEAR haSH".to_owned()),
                value: None
            })
        );

        Ok(())
    }
}

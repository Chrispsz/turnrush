//! Parser JSON mínimo — **std only, zero dependências**.
//!
//! Suficiente para o contrato de dados v2 do roster (objetos, arrays,
//! strings com escapes Unicode, inteiros, bool e null). Números fracionários
//! não fazem parte do contrato → o parser rejeita com mensagem clara.

/// Valor JSON genérico.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Busca um campo de objeto.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Json::Str(s) = self { Some(s) } else { None }
    }

    pub fn as_int(&self) -> Option<i64> {
        if let Json::Int(i) = self { Some(*i) } else { None }
    }

    pub fn as_arr(&self) -> Option<&[Json]> {
        if let Json::Arr(a) = self { Some(a) } else { None }
    }

    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        if let Json::Obj(o) = self { Some(o) } else { None }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "bool",
            Json::Int(_) => "int",
            Json::Str(_) => "string",
            Json::Arr(_) => "array",
            Json::Obj(_) => "objeto",
        }
    }
}

/// Analisa `src` como JSON (um documento completo).
pub fn parse(src: &str) -> Result<Json, String> {
    let bytes = src.as_bytes();
    let mut p = Parser { b: bytes, pos: 0 };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos != bytes.len() {
        return Err(p.err("conteúdo extra após o fim do JSON"));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, msg: &str) -> String {
        let start = self.pos.saturating_sub(24);
        let end = (self.pos + 24).min(self.b.len());
        let ctx = String::from_utf8_lossy(&self.b[start..end]);
        format!("JSON inválido @byte {}: {msg} (contexto: “{ctx}”)", self.pos)
    }

    fn skip_ws(&mut self) {
        while self.pos < self.b.len()
            && matches!(self.b[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(&format!("esperado '{}'", c as char)))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.err("valor inesperado")),
            None => Err(self.err("fim inesperado do JSON")),
        }
    }

    fn lit(&mut self, word: &str, v: Json) -> Result<Json, String> {
        if self.b[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(v)
        } else {
            Err(self.err(&format!("literal inválido (esperado '{word}')")))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let dig_start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == dig_start {
            return Err(self.err("número sem dígitos"));
        }
        if matches!(self.peek(), Some(b'.') | Some(b'e') | Some(b'E')) {
            return Err(self.err(
                "número fracionário/exponencial não faz parte do contrato (esperado inteiro)",
            ));
        }
        let s = std::str::from_utf8(&self.b[start..self.pos])
            .map_err(|_| self.err("utf8 inválido em número"))?;
        s.parse::<i64>()
            .map(Json::Int)
            .map_err(|_| self.err("inteiro fora do intervalo i64"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(self.err("string sem fechamento")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'"') => {
                            out.push('"');
                            self.pos += 1;
                        }
                        Some(b'\\') => {
                            out.push('\\');
                            self.pos += 1;
                        }
                        Some(b'/') => {
                            out.push('/');
                            self.pos += 1;
                        }
                        Some(b'n') => {
                            out.push('\n');
                            self.pos += 1;
                        }
                        Some(b't') => {
                            out.push('\t');
                            self.pos += 1;
                        }
                        Some(b'r') => {
                            out.push('\r');
                            self.pos += 1;
                        }
                        Some(b'b') => {
                            out.push('\u{8}');
                            self.pos += 1;
                        }
                        Some(b'f') => {
                            out.push('\u{c}');
                            self.pos += 1;
                        }
                        Some(b'u') => {
                            self.pos += 1;
                            let c = self.unicode_escape()?;
                            out.push(c);
                        }
                        _ => return Err(self.err("escape inválido")),
                    }
                }
                Some(_) => {
                    // consome um char UTF-8 completo
                    let rest = std::str::from_utf8(&self.b[self.pos..])
                        .map_err(|_| self.err("utf8 inválido"))?;
                    let Some(c) = rest.chars().next() else {
                        return Err(self.err("utf8 inválido"));
                    };
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<char, String> {
        let hi = self.hex4()?;
        if (0xD800..0xDC00).contains(&hi) {
            // par surrogate
            if self.b[self.pos..].starts_with(b"\\u") {
                self.pos += 2;
            } else {
                return Err(self.err("surrogate alto sem par \\u"));
            }
            let lo = self.hex4()?;
            if !(0xDC00..0xE000).contains(&lo) {
                return Err(self.err("surrogate baixo inválido"));
            }
            let cp = 0x1_0000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
            char::from_u32(cp).ok_or_else(|| self.err("codepoint inválido"))
        } else if (0xDC00..0xE000).contains(&hi) {
            Err(self.err("surrogate baixo isolado"))
        } else {
            char::from_u32(hi).ok_or_else(|| self.err("codepoint inválido"))
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        if self.pos + 4 > self.b.len() {
            return Err(self.err("\\u incompleto"));
        }
        let s = std::str::from_utf8(&self.b[self.pos..self.pos + 4])
            .map_err(|_| self.err("\\u inválido"))?;
        let v = u32::from_str_radix(s, 16).map_err(|_| self.err("\\u hexadecimal inválido"))?;
        self.pos += 4;
        Ok(v)
    }

    fn array(&mut self) -> Result<Json, String> {
        self.eat(b'[')?;
        let mut out = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(out));
        }
        loop {
            self.skip_ws();
            out.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(out));
                }
                _ => return Err(self.err("esperado ',' ou ']'")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.eat(b'{')?;
        let mut out: Vec<(String, Json)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(out));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.eat(b':')?;
            self.skip_ws();
            let v = self.value()?;
            out.push((key, v));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(out));
                }
                _ => return Err(self.err("esperado ',' ou '}'")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrato_v2_minimo() {
        let src = r#"{"version":2,"characters":[{"id":"a","name":"A","faction":"ember","role":"striker","hp":100,"attack":20,"defense":10,"speed":10,"mystery":{"name":"M","chakra":4,"cooldown":3,"targets":"enemies_front","damage_pct":120,"applies_state":"launch","effect":""},"standard":{"name":"S","damage_pct":90,"applies_state":"none"},"chase":{"name":"C","trigger":"launch","damage_pct":80,"applies_state":"knockback","max_per_round":1},"passive":{"name":"P","effect":"atk+10"}}]}"#;
        let v = parse(src).expect("parse do contrato");
        assert_eq!(v.get("version").and_then(Json::as_int), Some(2));
        let chars = v.get("characters").and_then(Json::as_arr).unwrap();
        assert_eq!(chars.len(), 1);
        let c = &chars[0];
        assert_eq!(c.get("faction").and_then(Json::as_str), Some("ember"));
        assert_eq!(c.get("mystery").map(|m| m.get("targets")).and_then(|t| t.and_then(Json::as_str)), Some("enemies_front"));
    }

    #[test]
    fn escapes_e_unicode() {
        let v = parse(r#""a\"b\\c\nAção""#).expect("escapes");
        assert_eq!(v.as_str(), Some("a\"b\\c\nAção"));
        let v2 = parse(r#""emoji: 😀""#).expect("surrogate pair");
        assert_eq!(v2.as_str(), Some("emoji: 😀"));
    }

    #[test]
    fn arrays_objetos_aninhados() {
        let v = parse(r#"{"a":[1,{"b":[true,false,null]},-7]}"#).unwrap();
        let a = v.get("a").and_then(Json::as_arr).unwrap();
        assert_eq!(a[0].as_int(), Some(1));
        assert_eq!(a[2].as_int(), Some(-7));
        assert_eq!(a[1].get("b").and_then(Json::as_arr).map(|b| b.len()), Some(3));
    }

    #[test]
    fn erros_com_mensagem_clara() {
        assert!(parse("").unwrap_err().contains("fim inesperado"));
        assert!(parse("{").unwrap_err().contains("JSON inválido"));
        assert!(parse("{\"a\":1.5}").unwrap_err().contains("inteiro"));
        assert!(parse("{\"a\":tru}").unwrap_err().contains("literal"));
        assert!(parse("[1,]").unwrap_err().contains("esperado"));
        assert!(parse("{}x").unwrap_err().contains("conteúdo extra"));
    }
}

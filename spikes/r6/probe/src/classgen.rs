//! Gerador mínimo de arquivos `.class` (versão 52, Java 8), sem javac.
//!
//! Serve só para montar classes sintéticas de teste. Nenhuma delas é executada
//! (não há Java nesta máquina); o objetivo é reproduzir o *formato* do bytecode
//! que os detectores procuram.

use std::collections::HashMap;

#[derive(Default)]
pub struct Pool {
    bytes: Vec<u8>,
    count: u16,
    cache: HashMap<String, u16>,
}

impl Pool {
    fn push(&mut self, key: String, entry: Vec<u8>) -> u16 {
        if let Some(i) = self.cache.get(&key) {
            return *i;
        }
        self.count += 1;
        let idx = self.count;
        self.bytes.extend(entry);
        self.cache.insert(key, idx);
        idx
    }
    pub fn utf8(&mut self, s: &str) -> u16 {
        let mut e = vec![1u8];
        e.extend((s.len() as u16).to_be_bytes());
        e.extend(s.as_bytes());
        self.push(format!("u:{s}"), e)
    }
    pub fn class(&mut self, name: &str) -> u16 {
        let n = self.utf8(name);
        let mut e = vec![7u8];
        e.extend(n.to_be_bytes());
        self.push(format!("c:{name}"), e)
    }
    pub fn string(&mut self, s: &str) -> u16 {
        let n = self.utf8(s);
        let mut e = vec![8u8];
        e.extend(n.to_be_bytes());
        self.push(format!("s:{s}"), e)
    }
    fn name_and_type(&mut self, name: &str, desc: &str) -> u16 {
        let n = self.utf8(name);
        let d = self.utf8(desc);
        let mut e = vec![12u8];
        e.extend(n.to_be_bytes());
        e.extend(d.to_be_bytes());
        self.push(format!("nt:{name}:{desc}"), e)
    }
    pub fn method(&mut self, owner: &str, name: &str, desc: &str) -> u16 {
        let c = self.class(owner);
        let nt = self.name_and_type(name, desc);
        let mut e = vec![10u8];
        e.extend(c.to_be_bytes());
        e.extend(nt.to_be_bytes());
        self.push(format!("m:{owner}.{name}{desc}"), e)
    }
}

/// Montador de bytecode com as poucas instruções que os testes usam.
pub struct Code<'a> {
    pub pool: &'a mut Pool,
    pub out: Vec<u8>,
}

impl<'a> Code<'a> {
    pub fn op(&mut self, b: u8) -> &mut Self {
        self.out.push(b);
        self
    }
    fn u16(&mut self, v: u16) {
        self.out.extend(v.to_be_bytes());
    }
    pub fn int(&mut self, v: i32) -> &mut Self {
        match v {
            -1..=5 => self.op((0x03 + v) as u8),
            -128..=127 => {
                self.op(0x10);
                self.out.push(v as i8 as u8);
                self
            }
            _ => {
                self.op(0x11);
                self.u16(v as i16 as u16);
                self
            }
        }
    }
    pub fn new_(&mut self, class: &str) -> &mut Self {
        let c = self.pool.class(class);
        self.op(0xbb);
        self.u16(c);
        self
    }
    pub fn anewarray(&mut self, class: &str) -> &mut Self {
        let c = self.pool.class(class);
        self.op(0xbd);
        self.u16(c);
        self
    }
    pub fn checkcast(&mut self, class: &str) -> &mut Self {
        let c = self.pool.class(class);
        self.op(0xc0);
        self.u16(c);
        self
    }
    pub fn ldc_string(&mut self, s: &str) -> &mut Self {
        let i = self.pool.string(s);
        self.ldc(i)
    }
    pub fn ldc_class(&mut self, class: &str) -> &mut Self {
        let i = self.pool.class(class);
        self.ldc(i)
    }
    fn ldc(&mut self, i: u16) -> &mut Self {
        if i < 256 {
            self.op(0x12);
            self.out.push(i as u8);
        } else {
            self.op(0x13);
            self.u16(i);
        }
        self
    }
    pub fn invoke(&mut self, kind: u8, owner: &str, name: &str, desc: &str) -> &mut Self {
        let m = self.pool.method(owner, name, desc);
        self.op(kind);
        self.u16(m);
        self
    }
    pub fn invokespecial(&mut self, o: &str, n: &str, d: &str) -> &mut Self {
        self.invoke(0xb7, o, n, d)
    }
    pub fn invokestatic(&mut self, o: &str, n: &str, d: &str) -> &mut Self {
        self.invoke(0xb8, o, n, d)
    }
    pub fn invokevirtual(&mut self, o: &str, n: &str, d: &str) -> &mut Self {
        self.invoke(0xb6, o, n, d)
    }
    /// `new String(new byte[]{…})` exatamente como o javac gera.
    pub fn string_from_bytes(&mut self, s: &str) -> &mut Self {
        self.new_("java/lang/String").op(0x59);
        self.int(s.len() as i32).op(0xbc);
        self.out.push(8); // T_BYTE
        for (i, b) in s.bytes().enumerate() {
            self.op(0x59);
            self.int(i as i32);
            self.int(b as i8 as i32);
            self.op(0x54); // bastore
        }
        self.invokespecial("java/lang/String", "<init>", "([B)V")
    }
}

pub struct Method {
    pub name: String,
    pub desc: String,
    pub code: Vec<u8>,
    pub max_stack: u16,
    pub max_locals: u16,
}

/// Monta a classe com construtor padrão e os métodos estáticos dados.
pub fn build_class(name: &str, mut pool: Pool, methods: Vec<Method>) -> Vec<u8> {
    let this = pool.class(name);
    let sup = pool.class("java/lang/Object");
    let code_attr = pool.utf8("Code");
    let init_ref = pool.method("java/lang/Object", "<init>", "()V");
    let init_n = pool.utf8("<init>");
    let init_d = pool.utf8("()V");
    let mut ms: Vec<(u16, u16, u16, Vec<u8>, u16, u16)> = vec![(
        0x0001,
        init_n,
        init_d,
        {
            let mut c = vec![0x2a, 0xb7];
            c.extend(init_ref.to_be_bytes());
            c.push(0xb1);
            c
        },
        1,
        1,
    )];
    for m in methods {
        let n = pool.utf8(&m.name);
        let d = pool.utf8(&m.desc);
        ms.push((0x0008 | 0x0002, n, d, m.code, m.max_stack, m.max_locals));
    }
    let mut out = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52];
    out.extend((pool.count + 1).to_be_bytes());
    out.extend(&pool.bytes);
    out.extend(0x0021u16.to_be_bytes()); // public super
    out.extend(this.to_be_bytes());
    out.extend(sup.to_be_bytes());
    out.extend(0u16.to_be_bytes()); // interfaces
    out.extend(0u16.to_be_bytes()); // fields
    out.extend((ms.len() as u16).to_be_bytes());
    for (flags, n, d, code, stack, locals) in ms {
        out.extend(flags.to_be_bytes());
        out.extend(n.to_be_bytes());
        out.extend(d.to_be_bytes());
        out.extend(1u16.to_be_bytes());
        out.extend(code_attr.to_be_bytes());
        let len = 2 + 2 + 4 + code.len() as u32 + 2 + 2;
        out.extend(len.to_be_bytes());
        out.extend(stack.to_be_bytes());
        out.extend(locals.to_be_bytes());
        out.extend((code.len() as u32).to_be_bytes());
        out.extend(code);
        out.extend(0u16.to_be_bytes()); // exception table
        out.extend(0u16.to_be_bytes()); // attributes
    }
    out.extend(0u16.to_be_bytes()); // class attributes
    out
}

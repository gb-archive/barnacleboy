use std::collections::HashMap;
use std::io;
use std::io::Write;
use interconnect::Interconnect;
use cpu::Cpu;

pub struct Debugger {
    breakpoints: HashMap<u16, u8>,
}

macro_rules! multibyte_command {
    ($args:ident, arg1 $a1:block arg2 $a2:block arg3 $a3:block) => (
        let len = $args.len();
        if len == 1 $a1
        else if len == 2 $a2
        else $a3
    )
}

impl Debugger {
    pub fn new() -> Debugger {
        Debugger {
            breakpoints: HashMap::new(),
        }
    }

    fn set_breakpoint(&mut self, ic: &mut Interconnect, addr: u16) -> Result<(), ()> {
        if !self.breakpoints.contains_key(&addr) {
            self.breakpoints.insert(addr, ic.mem_read_byte(addr));
            ic.mem_write_byte(addr, 0xDB);
            Ok(())
        } else {
            Err(())
        }
    }

    fn remove_breakpoint(&mut self, ic: &mut Interconnect, addr: u16) -> Result<(), ()> {
        if self.breakpoints.contains_key(&addr) {
            let value = self.breakpoints.remove(&addr);
            ic.mem_write_byte(addr, value.unwrap());
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn enter_debug_mode(
        &mut self,
        ic: &mut Interconnect,
        cpu: &mut Cpu,
        addr: u16,
        bkpt_triggered: bool,
    ) {
        if bkpt_triggered {
            if let Err(()) = self.remove_breakpoint(ic, addr) {
                println!("[X] Couldn't remove breakpoint that was triggered?");
            }
        }
        'dbglp: loop {
            print!("> ");
            let _ = io::stdout().flush();
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => {
                    if let Err(()) = self.execute(ic, cpu, &input) {
                        break 'dbglp;
                    }

                    let _ = io::stdout().flush();
                }
                Err(e) => {
                    println!("[X] Error reading line: {}", e);
                    break 'dbglp;
                }
            };
        }
    }

    fn execute(&mut self, ic: &mut Interconnect, cpu: &mut Cpu, input: &String) -> Result<(), ()> {
        let args: Vec<&str> = input.split(' ').map(|x| x.trim()).collect();

        match args[0] {
            "sbp" => {
                if &args[1][0..2] == "0x" {
                    if let Ok(num) = u16::from_str_radix(&args[1][2..], 16) {
                        if let Err(_) = self.set_breakpoint(ic, num) {
                            println!("[X] Breakpoint already exists at {:04X}", num);
                        }
                    } else {
                        println!("[X] Couldn't parse address");
                    }
                } else {
                    if let Ok(num) = args[1].parse() {
                        if let Err(_) = self.set_breakpoint(ic, num) {
                            println!("[X] Breakpoint already exists at {:04X}", num);
                        }
                    } else {
                        println!("[X] Couldn't parse address");
                    }
                }

                Ok(())
            }
            "rbp" => {
                if &args[1][0..2] == "0x" {
                    if let Ok(num) = u16::from_str_radix(&args[1][2..], 16) {
                        if let Err(_) = self.remove_breakpoint(ic, num) {
                            println!("[X] No breakpoint at {:04x} to remove", num);
                        }
                    } else {
                        println!("[X] Couldn't parse address");
                    }
                } else {
                    if let Ok(num) = args[1].parse() {
                        if let Err(_) = self.remove_breakpoint(ic, num) {
                            println!("[X] No breakpoint at {:04x} to remove", num);
                        }
                    } else {
                        println!("[X] Couldn't parse address");
                    }
                }
                Ok(())
            }
            "pbp" => {
                for (k, _) in &self.breakpoints {
                    println!("[B] 0x{:04X}", k);
                }
                Ok(())
            }
            "dasm" => {
                multibyte_command!(args,
                    arg1 {
                        println!("0x{:X}: {}", cpu.pc, Debugger::disassemble(ic, cpu.pc).0);
                    }
                    arg2 {
                        let mut offset = 0;
                        if let Ok(start) = u16::from_str_radix(&args[1][2..], 16) {
                            for i in 0..6 {
                                let (disas, length) = Debugger::disassemble(ic, start + offset);
                                println!("0x{:04X}: {}", start + offset, disas);
                                offset += length as u16;
                            }
                        }
                    }
                    arg3 {
                        let mut offset = 0;
                        if let Ok(num) = args[2].parse::<u16>() {
                            if let Ok(start) = u16::from_str_radix(&args[1][2..], 16) {
                                for i in 0..(num + 1) {
                                    let (disas, length) = Debugger::disassemble(ic, start + offset);
                                    println!("0x{:04X}: {}", start + offset, disas);
                                    offset += length as u16;
                                }
                            }
                        } else {
                            println!("[X] Couldn't parse number");
                        }
                });

                Ok(())
            }
            "md" | "memdisplay" => {
                if args.len() == 1 {
                    println!("[X] Missing location argument");
                } else if args.len() >= 2 {
                    if let Ok(start) = u16::from_str_radix(&args[1][2..], 16) {
                        print!("         00");

                        for i in 1..16 {
                            print!("  0{:01X}", i);
                        }

                        println!("");

                        let lines_to_print = match args.len() == 2 {
                            true => 5,
                            false => if let Ok(num) = args[2].parse::<u16>() {
                                num
                            } else {
                                5
                            },
                        };

                        for i in 0..(lines_to_print + 1) {
                            print!("0x{:04X}:  ", start + i * 16);
                            for k in 0..16 {
                                print!("{:02X}  ", ic.mem_read_byte(start + i * 16 + k));
                            }

                            println!("");
                        }
                    }
                }

                Ok(())
            }
            "s" | "step" => {
                println!("0x{:04X}: {}", cpu.pc, Debugger::disassemble(ic, cpu.pc).0);
                cpu.step(ic, false);
                Ok(())
            }
            "r" | "resume" => Err(()),
            _ => {
                println!("Unknown command");
                Ok(())
            }
        }
    }

    fn disassemble(ic: &mut Interconnect, addr: u16) -> (String, u8) {
        static R_STR: [&'static str; 8] = ["b", "c", "d", "e", "h", "l", "(hl)", "a"];

        static RP_STR: [&'static str; 4] = ["bc", "de", "hl", "sp"];

        static RP2_STR: [&'static str; 4] = ["bc", "de", "hl", "af"];

        static CC_STR: [&'static str; 4] = [" nz", " z", " nc", " c"];

        static ALU_STR: [&'static str; 8] = [
            "add a,",
            "adc a,",
            "sub",
            "sbc a,",
            "and",
            "xor",
            "or",
            "cp",
        ];

        static ROT_STR: [&'static str; 8] = ["rlc", "rrc", "rl", "rr", "sla", "sra", "swap", "srl"];

        let bytes = [
            ic.mem_read_byte(addr),
            ic.mem_read_byte(addr + 1),
            ic.mem_read_byte(addr + 2),
        ];
        let (x, y, z, p, q) = extract_x_y_z_p_q(bytes[0]);

        match (x, y, z, p, q) {
            // x = 0
            // match on y when z = 0
            (0, 0, 0, _, _) => (String::from("nop"), 1), //nop,

            (0, 1, 0, _, _) => (
                format!(
                    "ld (0x{:x}), sp",
                    ((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16))
                ),
                3,
            ),

            (0, 2, 0, _, _) => (String::from("stop"), 1),

            (0, 3, 0, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                let mut disas = String::from("jr");

                if y >= 4 && y <= 7 {
                    disas.push_str(CC_STR[y - 4]);
                }

                disas.push_str(&format!(" {}", bytes[1] as i16));

                (disas, 2)
            }

            (0, 4...7, 0, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                let mut disas = String::from("jr");

                if y >= 4 && y <= 7 {
                    disas.push_str(CC_STR[y - 4]);
                }

                disas.push_str(&format!(" {}", bytes[1] as i16));

                (disas, 2)
            }

            // match on q when z = 1
            (0, _, 1, _, 0) => {
                let p = extract_x_y_z_p_q(bytes[0]).3 as usize;
                (
                    format!(
                        "ld {}, 0x{:x}",
                        RP_STR[p],
                        (((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16)))
                    ),
                    3,
                )
            }

            (0, _, 1, _, 1) => {
                let p = extract_x_y_z_p_q(bytes[0]).3 as usize;
                (format!("add hl, {}", RP_STR[p]), 1)
            }

            // match on p when z = 2, q = 0
            (0, _, 2, 0, 0) => (String::from("ld (bc), a"), 1),

            (0, _, 2, 1, 0) => (String::from("ld (de), a"), 1),

            (0, _, 2, 2, 0) => (String::from("ld (hl+), a"), 1),

            (0, _, 2, 3, 0) => (String::from("ld (hl-), a"), 1),

            // match on p when z = 2, q = 1
            (0, _, 2, 0, 1) => (String::from("ld a, (bc)"), 1),

            (0, _, 2, 1, 1) => (String::from("ld a, (de)"), 1),

            (0, _, 2, 2, 1) => (String::from("ld a, (hl+)"), 1),

            (0, _, 2, 3, 1) => (String::from("ld a, (hl-)"), 1),

            // match on q when z = 3
            (0, _, 3, _, 0) => {
                let p = extract_x_y_z_p_q(bytes[0]).3 as usize;
                (format!("inc {}", RP_STR[p]), 1)
            }
            (0, _, 3, _, 1) => {
                let p = extract_x_y_z_p_q(bytes[0]).3 as usize;
                (format!("dec {}", RP_STR[p]), 1)
            }

            // match on z = 4 ... 6
            (0, _, 4, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                (format!("inc {}", R_STR[y]), 1)
            }

            (0, _, 5, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                (format!("dec {}", R_STR[y]), 1)
            }

            (0, _, 6, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                (format!("ld {}, 0x{:x}", R_STR[y], bytes[1]), 2)
            }

            // match on y when z = 7
            (0, 0, 7, _, _) => (String::from("rlca"), 1),

            (0, 1, 7, _, _) => (String::from("rrca"), 1),

            (0, 2, 7, _, _) => (String::from("rla"), 1),

            (0, 3, 7, _, _) => (String::from("rra"), 1),

            (0, 4, 7, _, _) => (String::from("daa"), 1),

            (0, 5, 7, _, _) => (String::from("cpl"), 1),

            (0, 6, 7, _, _) => (String::from("scf"), 1),

            (0, 7, 7, _, _) => (String::from("ccf"), 1),

            // x = 1
            // match on z = 0 ... 5 | 7
            (1, 6, 6, _, _) => (String::from("halt"), 1),
            (1, _, 0...7, _, _) => {
                let (_, y, z, _, _) = extract_x_y_z_p_q(bytes[0]);
                (
                    format!("ld {}, {}", R_STR[y as usize], R_STR[z as usize]),
                    1,
                )
            }

            /*(1, 6, 7, _, _) => {
                let (_, y, z, _, _) = extract_x_y_z_p_q(bytes[0]);
                format!("ld {}, {}", R_STR[y as usize], R_STR[z as usize])
            },*/

            // x = 2
            (2, _, _, _, _) => {
                let (_, y, z, _, _) = extract_x_y_z_p_q(bytes[0]);
                (format!("{} {}", ALU_STR[y as usize], R_STR[z as usize]), 1)
            }

            // x = 3
            // match on y when z = 0
            (3, 0...3, 0, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                (format!("ret {}", CC_STR[y]), 1)
            }

            (3, 4, 0, _, _) => (format!("ld 0xff00+0x{:x}, a", bytes[1]), 2),

            (3, 5, 0, _, _) => (format!("add sp, {}", bytes[1] as i8), 2),

            (3, 6, 0, _, _) => (format!("ld a, 0xff00+0x{:x}", bytes[1]), 2),

            (3, 7, 0, _, _) => (format!("ld hl, sp+{}", bytes[1] as i8), 2),

            // match on q = 0, z = 1
            (3, _, 1, _, 0) => {
                let p = extract_x_y_z_p_q(bytes[0]).3 as usize;
                (format!("pop {}", RP2_STR[p]), 1)
            }

            // match on p when q = 1, z = 1
            (3, _, 1, 0, 1) => (String::from("ret"), 1),
            (3, _, 1, 1, 1) => (String::from("reti"), 1),
            (3, _, 1, 2, 1) => (String::from("jp (hl)"), 1),
            (3, _, 1, 3, 1) => (String::from("ld sp, hl"), 1),

            // match on y when z = 2
            (3, 0...3, 2, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                (
                    format!(
                        "jp {} {}",
                        CC_STR[y],
                        ((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16))
                    ),
                    2,
                )
            }

            (3, 4, 2, _, _) => (String::from("ld (0xff00+c), a"), 1),

            (3, 5, 2, _, _) => (
                format!(
                    "ld ({:x}), a",
                    ((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16))
                ),
                3,
            ),
            (3, 6, 2, _, _) => (String::from("ld a, (0xff00+c)"), 1),
            (3, 7, 2, _, _) => (
                format!(
                    "ld a, ({:x})",
                    ((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16))
                ),
                3,
            ),

            // match on y when z = 3
            (3, 0, 3, _, _) => (
                format!(
                    "jp {:x}",
                    ((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16))
                ),
                3,
            ),

            (3, 1, 3, _, _) => {
                let (x, y, z, _, _) = extract_x_y_z_p_q(bytes[1]);

                match x {
                    0 => (format!("{} {}", ROT_STR[y as usize], R_STR[z as usize]), 2),
                    1 => (format!("bit {}, {}", y, R_STR[z as usize]), 2),
                    2 => (format!("res {}, {}", y, R_STR[z as usize]), 2),
                    3 => (format!("set {}, {}", y, R_STR[z as usize]), 2),
                    _ => (String::from("Unknown prefixed op"), 2),
                }
            }
            (3, 2...5, 3, _, _) => (String::from("nop"), 1),

            (3, 6, 3, _, _) => (String::from("di"), 1),

            (3, 7, 3, _, _) => (String::from("ei"), 1),

            // match on y when z = 4
            (3, 0...3, 4, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                (
                    format!(
                        "call {} 0x{:x}",
                        CC_STR[y],
                        ((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16))
                    ),
                    3,
                )
            }

            // match on q when z = 5
            (3, _, 5, _, 0) => {
                let p = extract_x_y_z_p_q(bytes[0]).3 as usize;
                (format!("push {}", RP2_STR[p]), 1)
            }
            (3, _, 5, 0, 1) => (
                format!(
                    "call 0x{:x}",
                    ((bytes[2] as u16) << 8) | (0x00FF & (bytes[1] as u16))
                ),
                3,
            ),

            // z = 6, 7
            (3, _, 6, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1 as usize;
                (format!("{} {:x}", ALU_STR[y], bytes[1]), 2)
            }

            (3, _, 7, _, _) => {
                let y = extract_x_y_z_p_q(bytes[0]).1;
                (format!("rst 0x{:x}", y * 8), 1)
            }

            // shouldn't get here, and if we do, something has gone very wrong
            _ => (String::from("nop"), 1),
        }
    }
}

fn extract_x_y_z_p_q(opcode: u8) -> (u8, u8, u8, u8, u8) {
    let (x, y, z) = (
        (opcode & 0b11000000) >> 6,
        (opcode & 0b00111000) >> 3,
        opcode & 0b00000111,
    );

    let (p, q) = ((y & 0b00000110) >> 1, y & 0b00000001);

    (x, y, z, p, q)
}

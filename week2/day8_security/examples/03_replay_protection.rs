//! Ejemplo 03 — Protección contra ataques de repetición con ventana deslizante
//!
//! Aunque HMAC previene la falsificación, un atacante puede grabar un TC válido y
//! enviarlo de nuevo más tarde (ataque de repetición). Una ventana de número de secuencia derrota esto.
//!
//! Ejecutar con:  cargo run --example 03_replay_protection

/// Protector de repetición con ventana deslizante de 64 paquetes.
pub struct ReplayWindow {
    last_seq: u16,
    /// Máscara de bits: el bit N establecido significa que seq (last_seq − N) ya fue aceptado.
    window: u64,
    initialized: bool,
}

#[derive(Debug, PartialEq)]
pub enum ReplayResult {
    Accept,
    Replay,
    TooOld,
}

impl ReplayWindow {
    pub fn new() -> Self { Self { last_seq: 0, window: 0, initialized: false } }

    pub fn check_and_advance(&mut self, seq: u16) -> ReplayResult {
        if !self.initialized {
            self.last_seq = seq;
            self.window = 1;
            self.initialized = true;
            return ReplayResult::Accept;
        }

        // Usar aritmética de 14 bits (el contador de seq CCSDS wrappea en 0x3FFF)
        let diff = (seq as i32 - self.last_seq as i32).rem_euclid(0x4000) as u16;

        if diff == 0 {
            return ReplayResult::Replay; // duplicado exacto
        }

        if diff <= 64 {
            // El paquete está adelante de nosotros — avanzar ventana
            self.window = self.window.wrapping_shl(diff as u32) | 1;
            self.last_seq = seq;
            ReplayResult::Accept
        } else if diff > 0x3FC0 {
            // El paquete está detrás (diff sería negativo en aritmética con signo)
            let back = (0x4000u32 - diff as u32) as usize;
            if back >= 64 { return ReplayResult::TooOld; }
            if self.window & (1u64 << back) != 0 { return ReplayResult::Replay; }
            self.window |= 1u64 << back;
            ReplayResult::Accept
        } else {
            // Muy adelante — gran salto en la secuencia (aceptar, avanzar ventana)
            self.window = 1;
            self.last_seq = seq;
            ReplayResult::Accept
        }
    }
}

fn main() {
    let mut window = ReplayWindow::new();

    println!("=== Demo de ventana de repetición ===\n");

    let scenarios: &[(u16, &str)] = &[
        (100, "primer paquete"),
        (101, "secuencial"),
        (102, "secuencial"),
        (104, "salto (103 perdido)"),
        (103, "fuera de orden pero dentro de la ventana"),
        (101, "REPETICIÓN de seq=101"),
        (100, "REPETICIÓN del seq=100 inicial"),
        (90,  "DEMASIADO ANTIGUO (> 64 detrás del actual)"),
        (105, "vuelta a la normalidad"),
    ];

    for &(seq, desc) in scenarios {
        let result = window.check_and_advance(seq);
        let status = match result {
            ReplayResult::Accept  => "ACEPTAR",
            ReplayResult::Replay  => "RECHAZAR (repetición)",
            ReplayResult::TooOld  => "RECHAZAR (demasiado antiguo)",
        };
        println!("  seq={seq:4}  [{status:30}]  {desc}");
    }

    println!();
    println!("El enfoque de ventana acepta paquetes fuera de orden dentro de 64 del");
    println!("último visto — necesario porque el enlace ascendente RF puede reordenar paquetes.");
    println!("Rechaza duplicados exactos y paquetes demasiado antiguos.");
}

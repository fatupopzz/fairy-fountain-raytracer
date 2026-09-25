//! Verificador independiente del ritmo de la cancion.
//!
//!     cargo run --release --bin analizar -- assets/music/loser.mp3
//!
//! NO ES EL ANALIZADOR DEL PROYECTO. El que alimenta la escena es
//! `analizar_audio.py`, que genera `loser_sync.json`; esto es una segunda
//! medicion, escrita sin librosa y con otro metodo, que sirve para una sola
//! cosa: comprobar que la primera no se equivoco.
//!
//! Y valio la pena tenerlo. La escena arrastraba un tempo de 109.065 BPM
//! escrito a mano, y las dos mediciones independientes coinciden en que el
//! tema va a 83: esto da 83.001 y librosa 83.35. El 109 era el artefacto
//! clasico de la autocorrelacion (el pico del tresillo, 83 x 4/3 = 110.6),
//! que aparece cuando se mira el pico mas alto sin puntuar sus multiplos.
//!
//! Se puede borrar sin que nada deje de compilar: no lo usa el raytracer.
//!
//! QUE SE MIDE, en orden:
//!   1. el flujo espectral, en tres bandas (grave / medio / agudo);
//!   2. el tempo, por autocorrelacion del flujo;
//!   3. los golpes, con programacion dinamica sobre el flujo (Ellis), que
//!      aguanta que el tempo se mueva un poco;
//!   4. el "uno" del compas, por la banda grave sobre esos golpes;
//!   5. los ATAQUES percusivos sueltos, que son los que disparan laseres;
//!   6. la envolvente de energia por banda, para lo continuo;
//!   7. los pozos (los frenos antes de cada coro), para cotejar la tabla de
//!      keyframes contra donde caen de verdad.

use std::f32::consts::PI;
use std::io::Write;
use std::process::Command;

/// Frecuencia a la que se analiza. 22050 alcanza y sobra: lo mas agudo que
/// interesa (los platos) vive bien por debajo de 11 kHz, y bajar a la mitad
/// del CD hace que todo lo demas cueste la mitad.
const SR: f32 = 22050.0;

/// Ventana de la FFT: 1024 muestras = 46 ms. Un compromiso clasico: mas
/// corta pierde los graves (a 1024 el bin mas bajo son 21 Hz), mas larga
/// emborrona el ataque de la percusion, que es justo lo que se busca.
const N: usize = 1024;

/// Salto entre ventanas: 256 muestras = 11.6 ms, o sea 86.13 cuadros por
/// segundo. Es la resolucion temporal de TODO lo que sale de aca.
const HOP: usize = 256;

/// Cuadros de analisis por segundo.
const FPS: f32 = SR / HOP as f32;

/// La envolvente que se guarda va mas lenta que el analisis: 40 por segundo
/// son 25 ms, de sobra para algo que se lee a 3 cuadros por segundo de
/// render, y hace la tabla el doble de chica.
const ENV_FPS: f32 = 40.0;

fn main() {
    let ruta = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/music/loser.mp3".to_string());
    let salida = std::env::args().nth(2).unwrap_or_else(|| "verificacion_ritmo.rs".to_string());

    let muestras = decodificar(&ruta);
    let duracion = muestras.len() as f32 / SR;
    println!("decodificado: {} muestras, {duracion:.3} s a {SR} Hz", muestras.len());

    let esp = espectrograma(&muestras);
    println!("espectrograma: {} cuadros x {} bins", esp.len(), N / 2);

    let bandas = flujo_por_bandas(&esp);
    let flujo = &bandas.total;

    let (periodo, pulso) = tempo(flujo);
    let bpm = 60.0 * FPS / periodo;
    let subdivision = (periodo / pulso).round() as usize;
    println!(
        "tempo: {bpm:.3} BPM (golpe {:.4} s), se sigue sobre el pulso de {:.4} s ({subdivision} por golpe)",
        periodo / FPS,
        pulso / FPS
    );

    let pulsos = seguir_golpes(flujo, pulso);
    println!("pulsos: {} seguidos", pulsos.len());
    informe_golpes(&pulsos, "pulso");

    // De los tics del pulso, los golpes son uno de cada `subdivision`. Cual
    // de las fases posibles se decide por donde pega el bombo.
    let golpes = quedarse_con_golpes(&pulsos, subdivision, &bandas.grave);
    println!("golpes: {} derivados del pulso", golpes.len());
    informe_golpes(&golpes, "golpe");

    let (base, periodo_medido, grilla_fija) = contra_grilla(&golpes);

    let fase_compas = fase_de_compas(&golpes, &bandas.grave);
    println!("compas: el \"uno\" cae en los golpes {fase_compas} mod 4");

    let ataques = picar_ataques(&bandas);
    println!("ataques percusivos: {}", ataques.len());
    verificar_con_ataques(&ataques, periodo);

    let env = envolventes(&bandas, duracion);
    let pozos = buscar_pozos(&env.medio, duracion);
    informe_estructura(&env, duracion, &pozos);

    escribir(
        &salida, &ruta, duracion, bpm, base, periodo_medido, grilla_fija, &golpes, fase_compas,
        &ataques, &env,
    );
    println!("\nescrito: {salida}");
}

// ============================================================
//  1. DECODIFICAR
// ============================================================

/// Le pide a ffmpeg el mp3 en crudo: mono, 22050, f32 little endian.
///
/// Se apoya en ffmpeg en vez de meter un decodificador de mp3 como
/// dependencia porque esto NO corre en el programa final: corre una vez, en
/// la maquina del que edita la escena. Meterle una caja de decodificacion al
/// binario que se entrega, para algo que ya esta precalculado, seria pagar
/// dos veces.
fn decodificar(ruta: &str) -> Vec<f32> {
    let salida = Command::new("ffmpeg")
        .args(["-v", "error", "-i", ruta, "-ac", "1", "-ar"])
        .arg(format!("{}", SR as u32))
        .args(["-f", "f32le", "-"])
        .output()
        .expect("no se pudo correr ffmpeg (brew install ffmpeg)");

    if !salida.status.success() {
        panic!("ffmpeg fallo: {}", String::from_utf8_lossy(&salida.stderr));
    }

    salida
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

// ============================================================
//  2. ESPECTROGRAMA
// ============================================================

/// FFT iterativa radix-2 in-place sobre (re, im).
///
/// Es la de manual, y esta escrita aca en vez de traida de una caja porque
/// son treinta lineas y la unica alternativa era sumar una dependencia mas
/// al proyecto para un programa que se corre una vez.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();

    // Permutacion de bits invertidos: deja las muestras en el orden en que
    // la etapa de mariposas las va a necesitar.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    let mut largo = 2;
    while largo <= n {
        let ang = -2.0 * PI / largo as f32;
        let (wr, wi) = (ang.cos(), ang.sin());

        for i in (0..n).step_by(largo) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..largo / 2 {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (vr, vi) = (
                    re[i + k + largo / 2] * cr - im[i + k + largo / 2] * ci,
                    re[i + k + largo / 2] * ci + im[i + k + largo / 2] * cr,
                );
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + largo / 2] = ur - vr;
                im[i + k + largo / 2] = ui - vi;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        largo <<= 1;
    }
}

/// Magnitud logaritmica por cuadro.
///
/// El logaritmo no es decoracion: la percepcion del volumen es logaritmica,
/// y sin el, el flujo espectral queda dominado por los momentos fuertes de
/// la cancion. Un golpe en el puente, que es igual de golpe pero suena mas
/// bajo, se perderia contra el coro.
fn espectrograma(x: &[f32]) -> Vec<Vec<f32>> {
    // Hann. Sin ventana, cada corte de 1024 muestras tiene dos bordes duros
    // que la FFT interpreta como energia de banda ancha, y el flujo se llena
    // de ataques que no existen.
    let ventana: Vec<f32> = (0..N)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / N as f32).cos())
        .collect();

    let cuadros = if x.len() > N { (x.len() - N) / HOP + 1 } else { 0 };
    let mut salida = Vec::with_capacity(cuadros);

    let mut re = vec![0.0f32; N];
    let mut im = vec![0.0f32; N];

    for c in 0..cuadros {
        let base = c * HOP;
        for i in 0..N {
            re[i] = x[base + i] * ventana[i];
            im[i] = 0.0;
        }
        fft(&mut re, &mut im);

        salida.push(
            (0..N / 2)
                .map(|k| (1.0 + 400.0 * (re[k] * re[k] + im[k] * im[k]).sqrt()).ln())
                .collect(),
        );
    }

    salida
}

// ============================================================
//  3. FLUJO ESPECTRAL POR BANDAS
// ============================================================

/// El flujo espectral: cuanta energia SUBIO de un cuadro al siguiente.
///
/// Solo lo que sube. Que la energia baje es que una nota se apaga, y eso no
/// es un ataque; contarlo pondria un pico en cada final de nota y dejaria la
/// curva con el doble de picos que golpes tiene la cancion.
struct Bandas {
    grave: Vec<f32>,
    medio: Vec<f32>,
    agudo: Vec<f32>,
    total: Vec<f32>,
}

/// Bin de la FFT en el que cae una frecuencia.
fn bin(hz: f32) -> usize {
    ((hz / SR * N as f32).round() as usize).min(N / 2 - 1)
}

fn flujo_por_bandas(esp: &[Vec<f32>]) -> Bandas {
    // Los tres cortes. Grave es el bombo, agudo son los platos y el hi-hat,
    // medio es la caja y todo lo demas. Los laseres cuelgan del agudo, que
    // es la banda que pega mas seguido.
    let (g0, g1) = (bin(25.0), bin(160.0));
    let (m0, m1) = (bin(160.0), bin(2000.0));
    let (a0, a1) = (bin(2000.0), bin(10000.0));

    let n = esp.len();
    let mut b = Bandas {
        grave: vec![0.0; n],
        medio: vec![0.0; n],
        agudo: vec![0.0; n],
        total: vec![0.0; n],
    };

    for c in 1..n {
        let (ant, act) = (&esp[c - 1], &esp[c]);
        let subida = |i0: usize, i1: usize| -> f32 {
            (i0..i1).map(|k| (act[k] - ant[k]).max(0.0)).sum::<f32>()
        };

        b.grave[c] = subida(g0, g1);
        b.medio[c] = subida(m0, m1);
        b.agudo[c] = subida(a0, a1);
        b.total[c] = subida(g0, a1);
    }

    normalizar(&mut b.grave);
    normalizar(&mut b.medio);
    normalizar(&mut b.agudo);
    normalizar(&mut b.total);
    b
}

/// Deja la curva con media 0 y desvio 1.
///
/// Hace falta para el seguidor de golpes: su penalizacion por salirse del
/// tempo esta calibrada contra un flujo en estas unidades. Con la curva sin
/// normalizar, el mismo numero de penalizacion es enorme en una cancion
/// tranquila e irrelevante en una fuerte.
fn normalizar(v: &mut [f32]) {
    let n = v.len().max(1) as f32;
    let media = v.iter().sum::<f32>() / n;
    let var = v.iter().map(|x| (x - media) * (x - media)).sum::<f32>() / n;
    let desvio = var.sqrt().max(1e-6);

    for x in v.iter_mut() {
        *x = (*x - media) / desvio;
    }
}

// ============================================================
//  4. TEMPO
// ============================================================

/// El periodo del pulso, en cuadros, por autocorrelacion del flujo.
///
/// Se busca entre 60 y 200 BPM, que es donde vive la musica bailable, y se
/// refina con una parabola sobre el pico: la autocorrelacion esta muestreada
/// cada 11.6 ms, y sin el refinamiento el error de medio cuadro se acumula
/// en varios golpes a lo largo de cuatro minutos.
///
/// EL PROBLEMA DE ESTE METODO, y por que abajo se puntua por armonicos: la
/// autocorrelacion no distingue un periodo de sus multiplos. Si el tema pega
/// cada 0.72 s, tambien correlaciona (menos, pero correlaciona) cada 1.44 y
/// cada 2.16. Sumar el valor en los primeros multiplos rompe el empate a
/// favor del periodo de verdad, que es el unico cuyos multiplos TODOS caen
/// sobre golpes.
///
/// Devuelve `(periodo_de_golpe, periodo_de_pulso)`, los dos en cuadros. El
/// pulso es el golpe partido al medio si el golpe es lento; ver
/// `elegir_pulso`.
fn tempo(flujo: &[f32]) -> (f32, f32) {
    let lag_min = (60.0 / 200.0 * FPS) as usize;
    let lag_max = (60.0 / 60.0 * FPS) as usize;

    let auto = |lag: usize| -> f32 {
        if lag >= flujo.len() {
            return 0.0;
        }
        let n = flujo.len() - lag;
        (0..n).map(|i| flujo[i] * flujo[i + lag]).sum::<f32>() / n as f32
    };

    // Puntaje por armonicos: el periodo mas sus tres multiplos, con peso
    // decreciente. Un candidato que sea el DOBLE del verdadero puntua bien
    // en si mismo pero mal en sus multiplos, porque estos caen entre golpes.
    let puntaje = |lag: usize| -> f32 {
        auto(lag) + 0.5 * auto(lag * 2) + 0.25 * auto(lag * 3) + 0.125 * auto(lag * 4)
    };

    let mut candidatos: Vec<(usize, f32)> = Vec::new();
    for lag in lag_min + 1..lag_max {
        let v = auto(lag);
        if v > auto(lag - 1) && v >= auto(lag + 1) {
            candidatos.push((lag, v));
        }
    }

    print!("  candidatos (BPM: autocorrelacion / con armonicos):");
    let mut ordenados = candidatos.clone();
    ordenados.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (lag, v) in ordenados.iter().take(6) {
        print!(" {:.1}({v:.3}/{:.3})", 60.0 * FPS / *lag as f32, puntaje(*lag));
    }
    println!();

    let mejor = candidatos
        .iter()
        .map(|(lag, _)| *lag)
        .max_by(|a, b| puntaje(*a).partial_cmp(&puntaje(*b)).unwrap())
        .unwrap_or(lag_min);

    // Parabola por los tres puntos alrededor del pico.
    let (y0, y1, y2) = (auto(mejor - 1), auto(mejor), auto(mejor + 1));
    let denom = y0 - 2.0 * y1 + y2;
    let ajuste = if denom.abs() > 1e-9 { 0.5 * (y0 - y2) / denom } else { 0.0 };

    let golpe = mejor as f32 + ajuste.clamp(-1.0, 1.0);
    (golpe, elegir_pulso(golpe))
}

/// El periodo sobre el que conviene CORRER el seguidor, que no siempre es el
/// del golpe.
///
/// Este tema pega cada 0.72 s, pero por debajo tiene una subdivision al
/// doble. Correr la programacion dinamica directo sobre 0.72 s la deja
/// eligiendo entre dos fases igual de buenas (los golpes y los contragolpes)
/// y el resultado sale con 20 ms de tembleque. Corriendola sobre la
/// subdivision, cada tic tiene un ataque de verdad debajo, el seguidor no
/// duda, y el golpe se recupera despues quedandose con uno de cada dos.
///
/// El corte en 0.62 s es donde deja de haber musica con el golpe tan
/// separado que el ojo lo lea como pulso: mas lento que eso siempre hay una
/// subdivision sonando.
fn elegir_pulso(golpe: f32) -> f32 {
    let mut p = golpe;
    while p / FPS > 0.62 {
        p /= 2.0;
    }
    p
}

/// Histograma de intervalos entre ataques: la comprobacion INDEPENDIENTE.
///
/// La autocorrelacion y el seguidor miran los dos la misma curva de flujo,
/// asi que si esa curva estuviera mal los dos se equivocarian igual y en
/// silencio. Esto mira otra cosa: los instantes ya detectados, y cuanto
/// suele haber entre uno y el siguiente. Si el tempo esta bien, los picos
/// del histograma caen en el periodo y en sus divisiones enteras.
fn verificar_con_ataques(ataques: &[(f32, f32)], periodo_golpe: f32) {
    if ataques.len() < 8 {
        return;
    }

    // Bins de 20 ms hasta 1.5 s.
    const BIN: f32 = 0.02;
    let n = (1.5 / BIN) as usize;
    let mut hist = vec![0u32; n];

    for par in ataques.windows(2) {
        let d = par[1].0 - par[0].0;
        let i = (d / BIN) as usize;
        if i < n {
            hist[i] += 1;
        }
    }

    let mut picos: Vec<(f32, u32)> = (1..n - 1)
        .filter(|&i| hist[i] > hist[i - 1] && hist[i] >= hist[i + 1] && hist[i] > 4)
        .map(|i| ((i as f32 + 0.5) * BIN, hist[i]))
        .collect();
    picos.sort_by(|a, b| b.1.cmp(&a.1));

    let golpe = periodo_golpe / FPS;
    println!("\nverificacion independiente (intervalos entre ataques):");
    println!("  golpe medido: {golpe:.4} s");
    for (d, c) in picos.iter().take(6) {
        let razon = golpe / d;
        println!(
            "  {d:.3} s x{c:<4}  = golpe / {razon:.2}{}",
            if (razon - razon.round()).abs() < 0.08 && razon.round() >= 1.0 {
                "   <- division entera del golpe"
            } else {
                ""
            }
        );
    }
}

// ============================================================
//  5. LOS GOLPES (programacion dinamica)
// ============================================================

/// Encuentra la secuencia de golpes que mejor explica el flujo.
///
/// Es el seguidor de Ellis. La idea: cada golpe puntua por la energia de
/// ataque que hay donde cae, MENOS lo que se aparta del periodo esperado. Se
/// resuelve entero, de una, con programacion dinamica, asi que no existe el
/// problema de los seguidores en linea (equivocarse en el golpe 40 y ya no
/// poder volver): la solucion es la mejor sobre la cancion COMPLETA.
///
/// Es tambien lo que hace que esto le gane a un BPM fijo con fase: si el
/// tema se corre medio golpe en el puente, la grilla queda corrida para
/// siempre y esto lo sigue.
fn seguir_golpes(flujo: &[f32], periodo: f32) -> Vec<f32> {
    let n = flujo.len();
    if n < 2 {
        return Vec::new();
    }

    // Cuanto pesa mantener el tempo contra cuanta energia hay. 100 es el
    // valor con el que se publico el metodo y anda: mas alto ignora la
    // musica y devuelve una grilla perfecta (que es lo que se quiere
    // evitar), mas bajo salta de corchea en corchea con cualquier adorno.
    const RIGIDEZ: f32 = 100.0;

    let mut puntaje = vec![f32::MIN; n];
    let mut previo = vec![usize::MAX; n];

    let desde = (periodo * 2.0).ceil() as usize;
    let hasta = (periodo * 0.5).floor().max(1.0) as usize;

    for t in 0..n {
        let mut mejor = 0.0f32; // arrancar la secuencia aca, sin previo
        let mut mejor_i = usize::MAX;

        let lo = t.saturating_sub(desde);
        let hi = t.saturating_sub(hasta);
        for tau in lo..=hi {
            if puntaje[tau] == f32::MIN {
                continue;
            }
            // Penalizacion log-cuadratica: es simetrica en RAZON, no en
            // diferencia. Adelantarse un 10% cuesta lo mismo que atrasarse
            // un 10%, que es como se percibe el ritmo.
            let razon = ((t - tau) as f32 / periodo).ln();
            let v = puntaje[tau] - RIGIDEZ * razon * razon;
            if v > mejor {
                mejor = v;
                mejor_i = tau;
            }
        }

        puntaje[t] = flujo[t] + mejor;
        previo[t] = mejor_i;
    }

    // Se arranca por el final: el mejor puntaje del ultimo tramo de la
    // cancion, no el mejor de todos, para no cortar la secuencia antes de
    // que el tema termine.
    let cola = n.saturating_sub((periodo * 2.0) as usize);
    let mut fin = cola;
    for t in cola..n {
        if puntaje[t] > puntaje[fin] {
            fin = t;
        }
    }

    let mut cuadros = Vec::new();
    let mut t = fin;
    while t != usize::MAX {
        cuadros.push(t);
        t = previo[t];
    }
    cuadros.reverse();

    cuadros.into_iter().map(|c| refinar(flujo, c)).collect()
}

/// Lleva un golpe del cuadro entero al instante de verdad.
///
/// La programacion dinamica solo puede devolver cuadros, y un cuadro dura
/// 11.6 ms: el golpe queda cuantizado a esa reja aunque el ataque haya caido
/// en el medio. Sobre un flash de laser de 60 ms, 11 ms de error es un
/// sexto del destello, y se nota como que el rig pega "gordo" en vez de
/// seco.
///
/// Se hacen dos cosas: se busca el maximo local del flujo a un cuadro de
/// distancia (el DP a veces elige el vecino porque le cerraba mejor el
/// tempo) y se le pasa una parabola a los tres puntos para leer donde
/// estaria el pico si el flujo estuviera muestreado mas fino.
fn refinar(flujo: &[f32], cuadro: usize) -> f32 {
    if cuadro == 0 || cuadro + 2 >= flujo.len() {
        return cuadro as f32 / FPS;
    }

    // Corregir de a un cuadro como mucho: mas que eso ya seria elegir otro
    // golpe, y esa decision es del DP, que ve la cancion entera.
    let c = (cuadro - 1..=cuadro + 1)
        .max_by(|a, b| flujo[*a].partial_cmp(&flujo[*b]).unwrap())
        .unwrap_or(cuadro);

    if c == 0 || c + 1 >= flujo.len() {
        return c as f32 / FPS;
    }

    let (y0, y1, y2) = (flujo[c - 1], flujo[c], flujo[c + 1]);
    let denom = y0 - 2.0 * y1 + y2;
    let ajuste = if denom.abs() > 1e-9 { 0.5 * (y0 - y2) / denom } else { 0.0 };

    (c as f32 + ajuste.clamp(-0.5, 0.5)) / FPS
}

/// Informe de estabilidad: cuanto se mueve el intervalo entre golpes.
///
/// Sirve para saber si valio la pena todo esto. Si el desvio fuera cero, una
/// grilla de BPM fijo alcanzaba; si es grande, la cancion respira y el
/// seguidor esta ganando algo real.
fn informe_golpes(golpes: &[f32], que: &str) {
    if golpes.len() < 3 {
        return;
    }

    let intervalos: Vec<f32> = golpes.windows(2).map(|w| w[1] - w[0]).collect();
    let media = intervalos.iter().sum::<f32>() / intervalos.len() as f32;
    let desvio = (intervalos.iter().map(|x| (x - media) * (x - media)).sum::<f32>()
        / intervalos.len() as f32)
        .sqrt();
    let peor = intervalos
        .iter()
        .map(|x| (x - media).abs())
        .fold(0.0f32, f32::max);

    println!(
        "  {que}: intervalo medio {media:.4} s ({:.2} BPM), desvio {:.1} ms, peor {:.1} ms",
        60.0 / media,
        desvio * 1000.0,
        peor * 1000.0
    );
    println!(
        "  primer golpe en {:.3} s, ultimo en {:.3} s",
        golpes[0],
        golpes[golpes.len() - 1]
    );

    // Cuanto se habria corrido una grilla fija respecto de los golpes de
    // verdad. Este numero es el argumento entero a favor del mapa.
    let deriva = golpes
        .iter()
        .enumerate()
        .map(|(i, g)| (g - (golpes[0] + i as f32 * media)).abs())
        .fold(0.0f32, f32::max);
    println!("  contra una grilla perfecta, se aparta hasta {:.0} ms", deriva * 1000.0);
}

/// Se queda con uno de cada `paso` tics del pulso: esos son los golpes.
///
/// La fase se elige por la banda grave, que es donde esta el bombo. Es el
/// mismo criterio que para el "uno" del compas, un nivel mas abajo.
fn quedarse_con_golpes(pulsos: &[f32], paso: usize, grave: &[f32]) -> Vec<f32> {
    if paso <= 1 {
        return pulsos.to_vec();
    }

    let energia = |fase: usize| -> f32 {
        pulsos
            .iter()
            .skip(fase)
            .step_by(paso)
            .map(|t| {
                let c = (t * FPS).round() as usize;
                // Una ventana chiquita, no el cuadro exacto: el bombo puede
                // caer 20 ms antes o despues del tic sin dejar de ser el uno.
                (c.saturating_sub(2)..=(c + 2))
                    .filter_map(|i| grave.get(i))
                    .fold(f32::MIN, |a, b| a.max(*b))
                    .max(0.0)
            })
            .sum()
    };

    let fase = (0..paso).max_by(|a, b| energia(*a).partial_cmp(&energia(*b)).unwrap()).unwrap_or(0);
    println!("  la fase del golpe dentro del pulso es {fase} de {paso}");

    pulsos.iter().skip(fase).step_by(paso).copied().collect()
}

/// Compara los golpes medidos contra la mejor grilla de tempo fijo.
///
/// La pregunta que contesta es si todo esto sirvio de algo. Si el tema esta
/// programado a tempo fijo (que es lo normal en musica electronica), los
/// golpes de verdad SON una grilla y las diferencias que mide el detector
/// son ruido suyo, no de la cancion: ahi conviene quedarse con la grilla
/// ajustada, que es la misma informacion sin el ruido. Si en cambio el
/// residuo se va para un lado a lo largo del tema, el tempo se mueve de
/// verdad y hay que guardar golpe por golpe.
///
/// El ajuste es por minimos cuadrados sobre `t_i = a + b * i`.
fn contra_grilla(golpes: &[f32]) -> (f32, f32, bool) {
    let n = golpes.len();
    if n < 4 {
        return (0.0, 0.0, false);
    }

    let nf = n as f32;
    let sx = (0..n).map(|i| i as f32).sum::<f32>();
    let sy = golpes.iter().sum::<f32>();
    let sxx = (0..n).map(|i| (i * i) as f32).sum::<f32>();
    let sxy = golpes.iter().enumerate().map(|(i, t)| i as f32 * t).sum::<f32>();

    let b = (nf * sxy - sx * sy) / (nf * sxx - sx * sx);
    let a = (sy - b * sx) / nf;

    let residuos: Vec<f32> = golpes
        .iter()
        .enumerate()
        .map(|(i, t)| t - (a + b * i as f32))
        .collect();

    let media = residuos.iter().sum::<f32>() / nf;
    let desvio = (residuos.iter().map(|r| (r - media) * (r - media)).sum::<f32>() / nf).sqrt();
    let peor = residuos.iter().fold(0.0f32, |m, r| m.max(r.abs()));

    println!("\ngolpes medidos contra la mejor grilla fija:");
    println!(
        "  grilla ajustada: primer golpe {a:.4} s, {:.4} s por golpe ({:.3} BPM)",
        b,
        60.0 / b
    );
    println!("  residuo: desvio {:.1} ms, peor {:.1} ms", desvio * 1000.0, peor * 1000.0);
    print!("  residuo medio por tramo de 40 s:");
    let por_tramo = (40.0 / b) as usize;
    let mut medias: Vec<f32> = Vec::new();
    for trozo in residuos.chunks(por_tramo.max(1)) {
        let m = trozo.iter().sum::<f32>() / trozo.len() as f32;
        medias.push(m);
        print!(" {:+.0}ms", m * 1000.0);
    }
    println!();

    // El veredicto. Lo que delata un tempo que se mueve no es que los golpes
    // sueltos se aparten (eso es ruido del detector y se promedia solo) sino
    // que el promedio de un tramo ENTERO se vaya para un lado. Con 30 ms de
    // corte: por debajo, ni el golpe mas ajustado de la escena se corre lo
    // suficiente como para que se vea; por encima, ya son 4 centesimas de
    // golpe acumuladas y hay que guardar tiempo por tiempo.
    let piso = medias.iter().cloned().fold(f32::MAX, f32::min);
    let techo = medias.iter().cloned().fold(f32::MIN, f32::max);
    let fija = techo - piso < 0.030;

    println!(
        "  el tempo {} (los tramos se separan {:.0} ms entre si)",
        if fija { "NO se mueve: alcanza con la grilla" } else { "SE MUEVE: hay que guardar golpe por golpe" },
        (techo - piso) * 1000.0
    );

    (a, b, fija)
}

/// Cual de cada cuatro golpes es el "uno".
///
/// El bombo cae en el uno y en el tres, y mas fuerte en el uno. Se prueban
/// las cuatro fases posibles y gana la que junta mas energia grave.
fn fase_de_compas(golpes: &[f32], grave: &[f32]) -> usize {
    let mut mejor = 0;
    let mut mejor_v = f32::MIN;

    for fase in 0..4 {
        let v: f32 = golpes
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 4 == fase)
            .map(|(_, g)| {
                let c = (g * FPS).round() as usize;
                grave.get(c).copied().unwrap_or(0.0)
            })
            .sum();
        if v > mejor_v {
            mejor_v = v;
            mejor = fase;
        }
    }

    mejor
}

// ============================================================
//  6. ATAQUES PERCUSIVOS
// ============================================================

/// Los instantes en que pega algo percusivo, con su fuerza.
///
/// No son los golpes: son TODOS los ataques, incluidos los que caen entre
/// golpe y golpe. De aca salen los disparos de los laseres, que es lo que
/// hace que el rig se mueva con la cancion y no con un metronomo.
///
/// El umbral es adaptativo (la mediana de una ventana movil): en el coro
/// hace falta mucha mas energia para contar como ataque que en el puente, y
/// asi el puente no se queda sin disparos ni el coro se llena de basura.
fn picar_ataques(b: &Bandas) -> Vec<(f32, f32)> {
    // El detector mira agudo y medio. El grave queda afuera a proposito: el
    // bombo ya mueve los anillos, y si tambien disparara laseres los dos
    // sistemas harian lo mismo al mismo tiempo y la escena perderia capas.
    let curva: Vec<f32> = b
        .agudo
        .iter()
        .zip(b.medio.iter())
        .map(|(a, m)| 0.7 * a + 0.3 * m)
        .collect();

    // Media segundo de ventana para la mediana.
    let radio = (FPS * 0.25) as usize;
    // Cuanto tiene que pasarse del fondo local. Medido: con 0.45 salen ~4
    // ataques por segundo en los coros y ~1 en el puente, que es la
    // densidad que se ve bien.
    const MARGEN: f32 = 0.45;
    // Nada de dos disparos a menos de 90 ms: mas junto que eso el ojo no lo
    // separa y el laser parpadea sucio.
    let separacion = (FPS * 0.09) as usize;

    let mut picos: Vec<(f32, f32)> = Vec::new();
    let mut ultimo = 0usize;
    let mut buffer: Vec<f32> = Vec::with_capacity(2 * radio + 1);

    for c in 1..curva.len() - 1 {
        // Un pico local, primero que nada.
        if curva[c] <= curva[c - 1] || curva[c] < curva[c + 1] {
            continue;
        }

        let lo = c.saturating_sub(radio);
        let hi = (c + radio).min(curva.len() - 1);
        buffer.clear();
        buffer.extend_from_slice(&curva[lo..hi]);
        buffer.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mediana = buffer[buffer.len() / 2];

        if curva[c] < mediana + MARGEN {
            continue;
        }
        if !picos.is_empty() && c - ultimo < separacion {
            // Si el nuevo pega mas fuerte, se queda con el lugar del
            // anterior en vez de perderse: en un redoble el golpe que
            // importa suele ser el ultimo.
            if curva[c] > picos[picos.len() - 1].1 {
                let n = picos.len() - 1;
                picos[n] = (c as f32 / FPS, curva[c]);
                ultimo = c;
            }
            continue;
        }

        picos.push((c as f32 / FPS, curva[c]));
        ultimo = c;
    }

    // La fuerza se normaliza a 0..1 contra el percentil 95, no contra el
    // maximo: un solo ataque enorme (un platillo del final) dejaria a todos
    // los demas en 0.2 y los laseres del resto de la cancion, apagados.
    let mut orden: Vec<f32> = picos.iter().map(|p| p.1).collect();
    orden.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let tope = orden
        .get((orden.len() as f32 * 0.95) as usize)
        .copied()
        .unwrap_or(1.0)
        .max(1e-3);

    picos
        .into_iter()
        .map(|(t, f)| (t, (f / tope).clamp(0.15, 1.0)))
        .collect()
}

// ============================================================
//  7. ENVOLVENTES
// ============================================================

struct Envolventes {
    grave: Vec<u8>,
    medio: Vec<u8>,
    agudo: Vec<u8>,
}

/// La energia sostenida de cada banda, a 40 por segundo y en un byte.
///
/// No es el flujo (que mide ataques) sino cuanta banda HAY: es lo que
/// distingue el puente del coro aunque los dos tengan golpes. Va en `u8`
/// porque el destino es un multiplicador visual: 256 escalones son mas de
/// los que el ojo separa en el brillo de un haz, y en f32 la tabla generada
/// pesaria cuatro veces mas.
fn envolventes(b: &Bandas, duracion: f32) -> Envolventes {
    let n = (duracion * ENV_FPS).ceil() as usize + 1;

    let una = |flujo: &[f32]| -> Vec<u8> {
        // Primero, energia acumulada en la ventana de cada cuadro de salida.
        let mut bruto = vec![0.0f32; n];
        for (c, &v) in flujo.iter().enumerate() {
            let i = ((c as f32 / FPS) * ENV_FPS) as usize;
            if i < n {
                // Solo lo positivo: el flujo esta centrado en cero y la
                // mitad negativa es "se apago algo", que no es energia.
                bruto[i] += v.max(0.0);
            }
        }

        // Suavizado exponencial de ida y de vuelta. Las dos pasadas son lo
        // que evita el corrimiento: un filtro de una sola direccion retrasa
        // la envolvente ~100 ms, y a 40 fps eso se ve como que la escena
        // reacciona tarde.
        const ALFA: f32 = 0.25;
        for i in 1..n {
            bruto[i] = bruto[i] * ALFA + bruto[i - 1] * (1.0 - ALFA);
        }
        for i in (0..n - 1).rev() {
            bruto[i] = bruto[i] * ALFA + bruto[i + 1] * (1.0 - ALFA);
        }

        let mut orden = bruto.clone();
        orden.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let tope = orden[(orden.len() as f32 * 0.97) as usize].max(1e-6);

        bruto
            .iter()
            .map(|v| ((v / tope).clamp(0.0, 1.0) * 255.0) as u8)
            .collect()
    };

    Envolventes {
        grave: una(&b.grave),
        medio: una(&b.medio),
        agudo: una(&b.agudo),
    }
}

// ============================================================
//  8. ESTRUCTURA
// ============================================================

/// Los pozos: tramos de al menos medio segundo con la energia por el piso.
///
/// Son los frenos antes de los coros, que es donde la tabla de keyframes
/// tiene sus filas mas delicadas. Que salgan medidos permite cotejar la
/// tabla contra el archivo en vez de confiar en el oido.
fn buscar_pozos(env: &[u8], duracion: f32) -> Vec<(f32, f32)> {
    let media = env.iter().map(|&v| v as f32).sum::<f32>() / env.len().max(1) as f32;
    let umbral = media * 0.35;
    let minimo = (ENV_FPS * 0.4) as usize;

    let mut pozos = Vec::new();
    let mut inicio: Option<usize> = None;

    for (i, &v) in env.iter().enumerate() {
        if (v as f32) < umbral {
            inicio.get_or_insert(i);
        } else if let Some(i0) = inicio.take() {
            if i - i0 >= minimo {
                pozos.push((i0 as f32 / ENV_FPS, i as f32 / ENV_FPS));
            }
        }
    }
    if let Some(i0) = inicio {
        if env.len() - i0 >= minimo {
            pozos.push((i0 as f32 / ENV_FPS, duracion));
        }
    }

    pozos
}

/// Los cortes de estructura: donde la cancion cambia de seccion.
///
/// El criterio es de contraste, no de nivel: para cada instante se compara
/// el promedio de las tres bandas en los ocho segundos ANTERIORES contra los
/// ocho SIGUIENTES. Donde esa diferencia hace un pico, cambio algo.
///
/// Ocho segundos porque a 83 BPM son casi tres compases: suficiente para que
/// una seccion quede caracterizada y corto para no borronear el borde. Con
/// ventanas de dos segundos, cada frase de la voz da un corte; con treinta,
/// el puente entero desaparece dentro del promedio.
fn secciones(env: &Envolventes, duracion: f32) -> Vec<f32> {
    let n = env.medio.len();
    let w = (ENV_FPS * 8.0) as usize;
    if n < 3 * w {
        return Vec::new();
    }

    let media = |v: &[u8], a: usize, b: usize| -> f32 {
        let (a, b) = (a.min(n), b.min(n));
        if b <= a {
            return 0.0;
        }
        v[a..b].iter().map(|&x| x as f32).sum::<f32>() / (b - a) as f32
    };

    let novedad: Vec<f32> = (0..n)
        .map(|i| {
            if i < w || i + w >= n {
                return 0.0;
            }
            let d = |v: &[u8]| (media(v, i, i + w) - media(v, i - w, i)).abs();
            // El grave pesa el doble: entrar o salir del bombo es EL cambio
            // de seccion en musica electronica, mucho mas que la voz.
            2.0 * d(&env.grave) + d(&env.medio) + d(&env.agudo)
        })
        .collect();

    // Picos, con un minimo de doce segundos entre cortes para que una
    // seccion no se parta en dos por un adorno.
    let separacion = (ENV_FPS * 12.0) as usize;
    let umbral = novedad.iter().sum::<f32>() / n as f32 * 1.6;

    let mut cortes: Vec<f32> = Vec::new();
    let mut ultimo = 0usize;
    for i in 1..n - 1 {
        if novedad[i] > umbral && novedad[i] >= novedad[i - 1] && novedad[i] > novedad[i + 1] {
            if cortes.is_empty() || i - ultimo >= separacion {
                cortes.push(i as f32 / ENV_FPS);
                ultimo = i;
            } else if novedad[i] > novedad[ultimo] {
                let k = cortes.len() - 1;
                cortes[k] = i as f32 / ENV_FPS;
                ultimo = i;
            }
        }
    }

    let _ = duracion;
    cortes
}

/// Imprime la forma de la cancion para contrastarla con la tabla a mano.
fn informe_estructura(env: &Envolventes, duracion: f32, pozos: &[(f32, f32)]) {
    println!("\nestructura medida (energia media por tramo de 5 s):");

    let paso = (ENV_FPS * 5.0) as usize;
    for (n, tramo) in env.medio.chunks(paso).enumerate() {
        let g: f32 = env.grave[(n * paso).min(env.grave.len())..]
            .iter()
            .take(tramo.len())
            .map(|&v| v as f32)
            .sum::<f32>()
            / tramo.len().max(1) as f32;
        let m = tramo.iter().map(|&v| v as f32).sum::<f32>() / tramo.len().max(1) as f32;
        let a: f32 = env.agudo[(n * paso).min(env.agudo.len())..]
            .iter()
            .take(tramo.len())
            .map(|&v| v as f32)
            .sum::<f32>()
            / tramo.len().max(1) as f32;

        let barra = "#".repeat((m / 255.0 * 40.0) as usize);
        println!(
            "  {:6.1}s  grave {:3.0}  medio {:3.0}  agudo {:3.0}  {barra}",
            n as f32 * 5.0,
            g,
            m,
            a
        );
    }

    println!("\npozos (frenos) de mas de 0.4 s, en {duracion:.1} s de tema:");
    for (a, b) in pozos {
        println!("  {a:6.2} .. {b:6.2}   ({:.2} s)", b - a);
    }

    let cortes = secciones(env, duracion);
    println!("\ncortes de estructura (donde cambia la instrumentacion):");
    print!("  0.0");
    for c in &cortes {
        print!("  {c:.1}");
    }
    println!("  {duracion:.1}");
}

// ============================================================
//  9. ESCRIBIR EL MODULO
// ============================================================

#[allow(clippy::too_many_arguments)]
fn escribir(
    salida: &str,
    ruta: &str,
    duracion: f32,
    bpm: f32,
    base: f32,
    periodo: f32,
    grilla_fija: bool,
    golpes: &[f32],
    fase: usize,
    ataques: &[(f32, f32)],
    env: &Envolventes,
) {
    let mut f = std::fs::File::create(salida).expect("no se pudo crear el archivo");

    let lista_f32 = |v: &[f32]| -> String {
        v.chunks(8)
            .map(|c| {
                let fila: Vec<String> = c.iter().map(|x| format!("{x:.4}")).collect();
                format!("    {},", fila.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let lista_u8 = |v: &[u8]| -> String {
        v.chunks(24)
            .map(|c| {
                let fila: Vec<String> = c.iter().map(|x| x.to_string()).collect();
                format!("    {},", fila.join(","))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let tiempos: Vec<f32> = ataques.iter().map(|a| a.0).collect();
    let fuerzas: Vec<f32> = ataques.iter().map(|a| a.1).collect();

    write!(
        f,
        r#"//! GENERADO POR `cargo run --release --bin analizar`. NO EDITAR A MANO.
//!
//! Fuente: {ruta}
//!
//! Es el ritmo de la cancion medido sobre el archivo, no supuesto: en que
//! segundo exacto cae cada golpe, en cuales pega algo percusivo y cuanta
//! energia tiene cada banda a lo largo del tema. `sync.rs` lo lee y de ahi
//! salen los anillos, los laseres y el bloom.
//!
//! Para regenerarlo despues de cambiar de cancion:
//!
//!     cargo run --release --bin analizar -- assets/music/otra.mp3

// Es una tabla de datos, no codigo: que una banda o una constante no se use
// hoy no significa que sobre, significa que la escena todavia no la
// aprovecha. Borrarlas obligaria a reanalizar para recuperarlas.
#![allow(dead_code)]

/// Largo del archivo analizado, en segundos.
pub const DURACION: f32 = {duracion:.3};

/// Tempo medio medido.
pub const BPM: f32 = {bpm:.3};

/// Si el tema esta a tempo fijo, o sea si la grilla de abajo lo describe
/// entero sin quedarse corta.
///
/// Cuando es `true`, `sync.rs` usa `GOLPE_BASE` y `GOLPE_PERIODO` y NO
/// `GOLPES`: la grilla ajustada por minimos cuadrados sobre los 300 y pico
/// de golpes detectados es mas precisa que cada deteccion suelta, porque
/// promedia el ruido del detector en vez de arrastrarlo.
pub const GRILLA_FIJA: bool = {grilla_fija};

/// Donde cae el primer golpe, en segundos desde el arranque del archivo.
pub const GOLPE_BASE: f32 = {base:.4};

/// Cuanto dura un golpe, en segundos.
pub const GOLPE_PERIODO: f32 = {periodo:.6};

/// Cual de cada cuatro golpes es el "uno" del compas.
pub const FASE_COMPAS: usize = {fase};

/// El instante de cada golpe detectado, en segundos y en orden.
///
/// Solo se usa si `GRILLA_FIJA` es `false`. Se guarda siempre igual: es el
/// dato crudo del que sale la grilla, y sin el no habria como comprobarla.
pub const GOLPES: [f32; {}] = [
{}
];

/// Instante de cada ataque percusivo (medios y agudos), en segundos.
///
/// ESTO es lo que dispara los laseres. No es una grilla: son los momentos en
/// que la cancion pega de verdad, incluidos los que caen entre golpe y
/// golpe, que son los que hacen que el rig se lea como que sigue al tema y
/// no como que corre un metronomo al lado.
pub const ATAQUES: [f32; {}] = [
{}
];

/// Fuerza de cada ataque, 0..1, en el mismo orden que `ATAQUES`.
pub const ATAQUES_FUERZA: [f32; {}] = [
{}
];

/// Cuadros por segundo de las tres envolventes.
pub const ENV_FPS: f32 = {ENV_FPS:.1};

/// Energia de graves (bombo), 0..255.
pub const ENV_GRAVE: [u8; {}] = [
{}
];

/// Energia de medios (caja, voz, cuerpo del tema), 0..255.
pub const ENV_MEDIO: [u8; {}] = [
{}
];

/// Energia de agudos (platos, hi-hat), 0..255.
pub const ENV_AGUDO: [u8; {}] = [
{}
];
"#,
        golpes.len(),
        lista_f32(golpes),
        tiempos.len(),
        lista_f32(&tiempos),
        fuerzas.len(),
        lista_f32(&fuerzas),
        env.grave.len(),
        lista_u8(&env.grave),
        env.medio.len(),
        lista_u8(&env.medio),
        env.agudo.len(),
        lista_u8(&env.agudo),
    )
    .expect("no se pudo escribir");
}

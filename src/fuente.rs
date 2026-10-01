//! LA FUENTE COMO EN OCARINA OF TIME.
//!
//! En el juego (la version original de N64) la Gran Fuente de las Hadas es
//! toda de baldosa de marmol blanco celeste: una piscina HEXAGONAL de agua
//! celeste, un pasillo que entra hasta una plataforma cuadrada con la
//! Trifuerza de oro incrustada en el piso (ahi se para Link a tocar), un
//! estrado en terrazas, dos antorchas enormes con forma de cono invertido y
//! fuego naranja a los costados, y una lluvia de brillos blancos y celestes
//! cayendo sobre todo. El Hada Mayor sale de la fuente.
//!
//! Este modulo arma todo eso sobre la isla:
//!   - la PISCINA hexagonal, con el borde abierto de frente para el pasillo
//!     y las columnas naciendo de sus seis esquinas;
//!   - el PASILLO y la PLATAFORMA de la Trifuerza, de baldosa blanca;
//!   - la TRIFUERZA del piso, de oro emisivo con su marco, que late con el
//!     golpe;
//!   - el ESTRADO hexagonal en terrazas con filetes de oro, y el CUENCO de
//!     agua de donde sale el hada, dentro de una FLOR DE LOTO (como en las
//!     fuentes de estilo egipcio de la version de 3DS);
//!   - las dos ANTORCHAS DE CONO con su fuego;
//!   - y la LLUVIA DE BRILLOS: ciento cincuenta gotas de luz que caen en un
//!     anillo alrededor del estrado, titilando, mas encendidas cuanto mas
//!     toca el arpa.

use crate::animacion::EscenaViva;
use crate::caja_orientada::CajaOrientada;
use crate::cube::Cube;
use crate::grupo_acotado::GrupoAcotado;
use crate::material::Material;
use crate::plane::{Limite, Plane};
use crate::ray_intersect::RayIntersect;
use crate::sphere::Sphere;
use crate::texture::{campo_fbm, Texture, TextureImage};
use crate::triangle::Triangle;
use crate::vec3::{cross, dot, Vec3};
use raylib::prelude::Color;
use std::f32::consts::PI;
use std::sync::Arc;

/// Medio ancho del pasillo.
pub const PASILLO: f32 = 0.65;
/// La altura del piso del pasillo: la misma que el escalon de arriba de la
/// entrada, asi Link camina derecho de la escalera al agua.
pub const PISO_PASILLO: f32 = 0.345;
/// Donde esta el centro de la Trifuerza del piso, sobre el pasillo.
const TRIFUERZA_Z: f32 = 2.25;
/// La altura del agua del cuenco: de ahi sale el hada, y de ahi sube la
/// columna de luz.
pub const CUENCO_Y: f32 = 0.84;
/// Donde van las dos antorchas de cono: a los costados del pasillo, sobre
/// el borde de la piscina, a la altura de Link.
pub const ANTORCHA_X: f32 = 1.4;
pub const ANTORCHA_Z: f32 = 3.0;
/// La altura de las llamas: de ahi salen las luces de las antorchas.
pub const LLAMA_Y: f32 = 2.4;
/// La piscina hexagonal: apotema del agua y del borde de afuera.
pub const PISCINA_ADENTRO: f32 = 2.65;
pub const PISCINA_AFUERA: f32 = 3.25;

/// Los materiales de la escena que la fuente reusa.
pub struct Materiales<'a> {
    pub oro: &'a Material,
    pub oro_sagrado: &'a Material,
    pub obsidiana: &'a Material,
    pub agua: &'a Material,
    pub fondo: &'a Material,
}

fn hash(x: i32, y: i32, k: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ k.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
}

fn img(t: TextureImage) -> Texture {
    Texture::ImageTexture(Arc::new(t), Color::WHITE, (0.0, 0.0))
}

/// La baldosa blanca celeste del juego: cuadros de marmol palido con la
/// junta apenas mas oscura y cada uno de un tono un poco distinto.
fn textura_baldosa_blanca() -> TextureImage {
    let ruido = campo_fbm(128, 6, 3, 421);
    TextureImage::pintada(128, 128, move |u, v| {
        let n = ruido[((v * 127.0) as usize) * 128 + (u * 127.0) as usize];
        let (cx, cy) = ((u * 2.0).floor() as i32, (v * 2.0).floor() as i32);
        let (fx, fy) = ((u * 2.0).fract(), (v * 2.0).fract());
        if fx < 0.03 || fy < 0.03 {
            return [0.52, 0.64, 0.68];
        }
        let tono = hash(cx, cy, 11) * 0.06;
        let k = 0.9 + 0.1 * n;
        [(0.72 - tono) * k, (0.82 - tono) * k, (0.85 - tono * 0.5) * k]
    })
}

/// EL MARMOL PERLADO del templete (columnas y techo): blanco con un toque
/// lavanda, con vetas suaves rosas y celestes que siguen el ruido. Antes era
/// el marmol teal de la piscina, y de noche las columnas y el techo se leian
/// como bloques casi negros tapando la fuente: una jaula en vez de un
/// templete.
pub fn textura_marmol_perla() -> TextureImage {
    let ruido = campo_fbm(256, 6, 4, 433);
    let vetas = campo_fbm(256, 3, 5, 437);
    TextureImage::pintada(256, 256, move |u, v| {
        let i = ((v * 255.0) as usize) * 256 + (u * 255.0) as usize;
        let n = ruido[i];
        // Las vetas: donde el segundo ruido cruza por la mitad, una linea
        // fina que se tuerce.
        let veta = 1.0 - (((vetas[i] - 0.5) * 22.0 + n * 3.0).sin().abs()).powf(0.25);
        let base = [0.74 + 0.06 * n, 0.72 + 0.05 * n, 0.82 + 0.04 * n];
        let tinte = if vetas[i] > 0.5 { [0.88, 0.58, 0.78] } else { [0.55, 0.74, 0.86] };
        let k = (veta * 0.55).clamp(0.0, 1.0);
        [
            base[0] + (tinte[0] - base[0]) * k,
            base[1] + (tinte[1] - base[1]) * k,
            base[2] + (tinte[2] - base[2]) * k,
        ]
    })
}

/// Un petalo de loto: rosa palido en la base, rosa intenso en la punta, con
/// nervaduras finas a lo largo. Las UV de la cometa ponen la base en v = 1 y
/// la punta en v = 0.
fn textura_petalo() -> TextureImage {
    TextureImage::pintada(64, 64, |u, v| {
        let sube = 1.0 - v;
        let nervio = 0.9 + 0.1 * ((u - 0.5) * 40.0).cos().abs();
        let (r, g, b) = (1.0, 0.78 - 0.38 * sube, 0.88 - 0.25 * sube);
        [r * nervio, g * nervio, b * nervio]
    })
}

/// El estado de la fuente que se mueve con la cancion.
pub struct FuenteViva {
    /// La lluvia de brillos que cae sobre la fuente: un grupo por gota.
    lluvia: Vec<usize>,
}

/// Cuantas gotas de brillo caen sobre la fuente.
const GOTAS: usize = 150;
/// De donde a donde caen.
const LLUVIA_TECHO: f32 = 7.5;
const LLUVIA_RADIO: f32 = 3.6;
const LLUVIA_ADENTRO: f32 = 1.7;

/// Un prisma HEXAGONAL hecho de cubos, con un lado mirando a +Z: tres cajas
/// iguales, de ancho igual a la distancia entre dos lados opuestos y de largo
/// igual al lado, giradas 60 grados entre si. Se superponen exactamente en el
/// hexagono: las esquinas de cada caja caen justo en vertices.
pub fn hexagono(apotema: f32, y0: f32, alto: f32, mat: &Material) -> Vec<Box<dyn RayIntersect + Send + Sync>> {
    let lado = 2.0 * apotema / 3.0f32.sqrt();
    (0..3)
        .map(|k| {
            let giro = PI / 2.0 + k as f32 * PI / 3.0;
            let mut caja = CajaOrientada::nueva(
                Vec3::new(0.0, y0 + alto * 0.5, 0.0),
                Vec3::new(apotema * 2.0, alto, lado),
                crate::caja_orientada::ejes_de(giro, 0.0, 0.0),
                mat.clone(),
            );
            caja.mosaico = 1.0;
            Box::new(caja) as Box<dyn RayIntersect + Send + Sync>
        })
        .collect()
}

/// El borde de un hexagono: seis listones, uno por lado. Si `hueco` es mayor
/// que cero, el lado de +Z se parte y deja un paso de ese medio ancho en el
/// medio (por ahi entra el pasillo).
pub fn borde_hexagonal(
    apotema: f32,
    grosor: f32,
    y0: f32,
    alto: f32,
    mat: &Material,
    hueco: f32,
) -> Vec<Box<dyn RayIntersect + Send + Sync>> {
    let lado = 2.0 * apotema / 3.0f32.sqrt();
    let mut piezas: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();
    for k in 0..6 {
        let a = PI / 2.0 + k as f32 * PI / 3.0;
        let n = Vec3::new(a.cos(), 0.0, a.sin());
        let arriba = Vec3::new(0.0, 1.0, 0.0);
        let t = cross(&arriba, &n);
        let centro = n * (apotema - grosor * 0.5) + Vec3::new(0.0, y0 + alto * 0.5, 0.0);
        let largo = lado + grosor * 0.6;
        let tramos: Vec<(f32, f32)> = if k == 0 && hueco > 0.0 {
            // Dos mitades, de `hueco` a la punta de cada lado.
            let medio = (largo * 0.5 - hueco) * 0.5 + hueco;
            vec![(-medio, largo * 0.5 - hueco), (medio, largo * 0.5 - hueco)]
        } else {
            vec![(0.0, largo)]
        };
        for (corrido, largo) in tramos {
            let mut caja = CajaOrientada::nueva(centro + t * corrido, Vec3::new(grosor, alto, largo), [n, arriba, t], mat.clone());
            caja.mosaico = 1.0;
            piezas.push(Box::new(caja));
        }
    }
    piezas
}

/// Arma la fuente de Ocarina sobre la piscina y la registra en la escena.
pub fn armar(
    objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>,
    escena: &mut EscenaViva,
    m: &Materiales,
) -> FuenteViva {
    // ---- LA BALDOSA BLANCA ----
    // Todo lo que pisa Link en el juego es de baldosa de marmol blanco
    // celeste: el pasillo, la plataforma de la Trifuerza, los bordes de la
    // piscina, el estrado y las antorchas.
    let blanca = Material::new([0.55, 0.30, 0.12, 0.0], 50.0, 0.0, img(textura_baldosa_blanca()), None)
        .con_rugosidad(0.07);

    // ---- LA PISCINA HEXAGONAL ----
    // Como la del juego: hexagonal, con un lado mirando a Link, y el borde de
    // ese lado abierto para que entre el pasillo. Los vertices caen justo
    // donde estan las seis columnas, que nacen de las esquinas del borde.
    let mut piscina: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();
    piscina.extend(hexagono(PISCINA_ADENTRO + 0.1, -0.42, 0.14, m.fondo));
    piscina.extend(borde_hexagonal(PISCINA_AFUERA, PISCINA_AFUERA - PISCINA_ADENTRO, -0.35, 1.0, &blanca, PASILLO));
    piscina.extend(borde_hexagonal(PISCINA_AFUERA - 0.12, 0.34, 0.64, 0.1, m.oro, PASILLO));
    piscina.push(Box::new(Plane {
        point: Vec3::new(0.0, 0.1, 0.0),
        normal: Vec3::new(0.0, 1.0, 0.0),
        ripple_center: Vec3::zeros(),
        ripple_strength: 0.08,
        ripple_scale: 3.0,
        ripple_phase: 0.0,
        uv_scale: 1.25,
        limite: Some(Limite::Hexagono(PISCINA_ADENTRO)),
        material: m.agua.clone(),
        ondas: Vec::new(),
        onda_color: (1.0, 0.45, 0.88),
    }));
    escena.registrar_agua(objetos.len(), piscina.len() - 1, m.agua.emission_color);
    objetos.push(Box::new(GrupoAcotado::estatico(piscina)));

    // ---- EL PASILLO Y LA PLATAFORMA DE LA TRIFUERZA ----
    let baldosa = blanca.clone();
    let (z0, z1) = (1.2f32, 3.35f32);
    let alto = PISO_PASILLO + 0.25;
    let mut pasillo: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
        Box::new(
            Cube::new_rect(
                Vec3::new(0.0, PISO_PASILLO - alto * 0.5, (z0 + z1) * 0.5),
                PASILLO * 2.0,
                alto,
                z1 - z0,
                baldosa.clone(),
            )
            .con_mosaico(1.0),
        ),
        // La plataforma cuadrada donde esta la Trifuerza, mas ancha que el
        // pasillo, metida en el agua: ahi se para Link en el juego.
        Box::new(
            Cube::new_rect(
                Vec3::new(0.0, PISO_PASILLO - alto * 0.5, TRIFUERZA_Z + 0.1),
                1.7,
                alto,
                1.7,
                baldosa,
            )
            .con_mosaico(1.0),
        ),
    ];
    // Los filetes de oro a los costados.
    for lado in [-1.0f32, 1.0] {
        pasillo.push(Box::new(Cube::new_rect(
            Vec3::new(lado * (PASILLO + 0.03), PISO_PASILLO - 0.02, (z0 + z1) * 0.5),
            0.07,
            0.08,
            z1 - z0,
            m.oro.clone(),
        )));
    }
    objetos.push(Box::new(GrupoAcotado::estatico(pasillo)));

    // ---- LA TRIFUERZA DEL PISO ----
    // Tres triangulos de oro emisivo acostados sobre el pasillo, con la
    // punta hacia el estrado. El orden de los vertices se corrige para que
    // la normal mire hacia arriba (el Triangle no la voltea).
    let lado = 0.46;
    let alto_tri = lado * 0.866;
    let base_z = TRIFUERZA_Z + alto_tri;
    let y = PISO_PASILLO + 0.004;
    let punto = |x: f32, sube: f32| Vec3::new(x, y, base_z - sube);
    let tri = |a: Vec3, b: Vec3, c: Vec3| {
        let n = cross(&(b - a), &(c - a));
        let (b, c) = if dot(&n, &Vec3::new(0.0, 1.0, 0.0)) < 0.0 { (c, b) } else { (b, c) };
        Box::new(Triangle { a, b, c, uv_a: None, uv_b: None, uv_c: None, material: Material { emission_color: Some(Color::new(105, 80, 20, 255)), ..m.oro_sagrado.clone() } })
            as Box<dyn RayIntersect + Send + Sync>
    };
    let triangulos = vec![
        tri(punto(-lado, 0.0), punto(0.0, 0.0), punto(-lado / 2.0, alto_tri)),
        tri(punto(0.0, 0.0), punto(lado, 0.0), punto(lado / 2.0, alto_tri)),
        tri(punto(-lado / 2.0, alto_tri), punto(lado / 2.0, alto_tri), punto(0.0, 2.0 * alto_tri)),
    ];
    // Mas apagada que la Trifuerza flotante de antes: esta en el piso, a los
    // pies de Link, y con la emision entera el halo le lavaba las piernas.
    let emision = Color::new(105, 80, 20, 255);
    // El marco cuadrado de oro en el que esta incrustada, como la baldosa del
    // juego: cuatro listones apenas levantados sobre el pasillo.
    let (mz, mm) = (TRIFUERZA_Z + 0.05, 0.56);
    let liston = |x: f32, z: f32, sx: f32, sz: f32| {
        Box::new(Cube::new_rect(Vec3::new(x, PISO_PASILLO + 0.005, z), sx, 0.02, sz, m.oro.clone()))
            as Box<dyn RayIntersect + Send + Sync>
    };
    objetos.push(Box::new(GrupoAcotado::estatico(vec![
        liston(0.0, mz - mm, mm * 2.0 + 0.06, 0.06),
        liston(0.0, mz + mm, mm * 2.0 + 0.06, 0.06),
        liston(-mm, mz, 0.06, mm * 2.0),
        liston(mm, mz, 0.06, mm * 2.0),
    ])));
    escena.registrar_triforce(objetos.len(), (0..3).map(|i| (i, emision)).collect());
    objetos.push(Box::new(GrupoAcotado::new(triangulos)));

    // ---- EL ESTRADO HEXAGONAL Y EL CUENCO ----
    // En el juego la fuente es un estrado HEXAGONAL en terrazas, no redondo
    // (ver `hexagono` y `borde_hexagonal`).
    let mut estrado: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();
    estrado.extend(hexagono(1.55, -0.3, 0.68, &blanca));
    estrado.extend(hexagono(1.60, 0.38, 0.04, m.oro));
    estrado.extend(hexagono(1.20, 0.42, 0.20, &blanca));
    estrado.extend(hexagono(1.24, 0.60, 0.03, m.oro));
    estrado.extend(hexagono(0.82, 0.60, 0.14, m.obsidiana));
    estrado.extend(borde_hexagonal(0.98, 0.18, 0.63, 0.24, &blanca, 0.0));
    estrado.extend(borde_hexagonal(1.00, 0.06, 0.86, 0.04, m.oro, 0.0));
    estrado.push(Box::new(Plane {
        point: Vec3::new(0.0, CUENCO_Y, 0.0),
        normal: Vec3::new(0.0, 1.0, 0.0),
        ripple_center: Vec3::zeros(),
        ripple_strength: 0.08,
        ripple_scale: 5.0,
        ripple_phase: 0.0,
        uv_scale: 1.25,
        limite: Some(Limite::Elipse(0.8, 0.8)),
        material: m.agua.clone(),
        ondas: Vec::new(),
        onda_color: (1.0, 0.45, 0.88),
    }));
    escena.registrar_agua(objetos.len(), estrado.len() - 1, m.agua.emission_color);
    objetos.push(Box::new(GrupoAcotado::estatico(estrado)));

    // ---- LA FLOR DE LOTO ----
    // Como en las fuentes de estilo egipcio de la version de 3DS, el cuenco
    // nace de una flor de loto: dos coronas de petalos rosas, la de afuera
    // abierta y la de adentro mas parada. Cada petalo es una cometa de dos
    // triangulos (base en el borde, dos costados y la punta), con un brillo
    // propio tenue: son lo que se ve alrededor del hada cuando sale.
    let petalo = Material::new(
        [1.0, 0.45, 0.10, 0.0],
        50.0,
        0.0,
        img(textura_petalo()),
        Some(Color::new(70, 18, 45, 255)),
    );
    let mut loto: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();
    for (corona, (cuantos, r0, r1, alto, ancho, giro)) in
        [(12usize, 0.95f32, 1.55f32, 0.55f32, 0.30f32, 0.0f32), (10, 0.88, 1.20, 0.75, 0.24, 0.3)].into_iter().enumerate()
    {
        let _ = corona;
        for k in 0..cuantos {
            let a = giro + k as f32 * 2.0 * PI / cuantos as f32;
            let (s, c) = a.sin_cos();
            let n = Vec3::new(c, 0.0, s);
            let t = Vec3::new(-s, 0.0, c);
            let base = n * r0 + Vec3::new(0.0, 0.86, 0.0);
            let punta = n * r1 + Vec3::new(0.0, 0.86 + alto, 0.0);
            let medio = base + (punta - base) * 0.45;
            let izq = medio - t * ancho;
            let der = medio + t * ancho;
            let tri = |a: Vec3, b: Vec3, c: Vec3| {
                // La normal hacia AFUERA y arriba de la flor.
                let nor = cross(&(b - a), &(c - a));
                let (b, c) = if dot(&nor, &(n + Vec3::new(0.0, 0.6, 0.0))) < 0.0 { (c, b) } else { (b, c) };
                Box::new(Triangle {
                    a,
                    b,
                    c,
                    uv_a: Some((0.5, 1.0)),
                    uv_b: Some((0.0, 0.5)),
                    uv_c: Some((0.5, 0.0)),
                    material: petalo.clone(),
                }) as Box<dyn RayIntersect + Send + Sync>
            };
            loto.push(tri(base, izq, punta));
            loto.push(tri(base, punta, der));
        }
    }
    objetos.push(Box::new(GrupoAcotado::estatico(loto)));

    // ---- LAS ANTORCHAS DE CONO ----
    // Las de la fuente del juego: dos conos invertidos enormes, como copas de
    // cristal, con fuego naranja arriba, uno a cada lado del pasillo donde
    // Link se para a tocar. El cono es una piramide hexagonal invertida de
    // cristal (refracta) sobre un pedestal, con la tapa de arriba y la llama.
    // De marmol blanco y opacas, como en el juego.
    let cristal_blanco = blanca.clone();
    let llama_naranja = Material::new(
        [1.0, 0.0, 0.0, 0.0],
        1.0,
        0.0,
        Texture::Solid(Color::new(255, 200, 120, 255)),
        // Algo menos que el naranja pleno: con todo, la llama se quemaba a
        // blanco y dejaba un manchon de halo en cada antorcha.
        Some(Color::new(215, 100, 25, 255)),
    );
    for lado in [-1.0f32, 1.0] {
        let c = Vec3::new(lado * ANTORCHA_X, 0.0, ANTORCHA_Z);
        let (abajo, arriba, radio) = (0.95f32, LLAMA_Y - 0.25, 0.58f32);
        let vertice = Vec3::new(c.x, abajo, c.z);
        let borde_de = |k: usize| {
            let a = k as f32 * PI / 3.0;
            Vec3::new(c.x + a.cos() * radio, arriba, c.z + a.sin() * radio)
        };
        let tapa = Vec3::new(c.x, arriba, c.z);
        // El pedestal, en dos escalones de baldosa y un dado de oro.
        let mut antorcha: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
            Box::new(Cube::new_rect(Vec3::new(c.x, 0.70, c.z), 0.95, 0.1, 0.75, blanca.clone()).con_mosaico(1.0)),
            Box::new(Cube::new_rect(Vec3::new(c.x, 0.80, c.z), 0.7, 0.1, 0.6, blanca.clone()).con_mosaico(1.0)),
            Box::new(Cube::new_rect(Vec3::new(c.x, 0.90, c.z), 0.3, 0.1, 0.3, m.oro.clone())),
        ];
        for k in 0..6 {
            let (p, q) = (borde_de(k), borde_de((k + 1) % 6));
            let lado_cono = |a: Vec3, b: Vec3, cc: Vec3, afuera: Vec3| {
                let nor = cross(&(b - a), &(cc - a));
                let (b, cc) = if dot(&nor, &afuera) < 0.0 { (cc, b) } else { (b, cc) };
                Box::new(Triangle { a, b, c: cc, uv_a: None, uv_b: None, uv_c: None, material: cristal_blanco.clone() })
                    as Box<dyn RayIntersect + Send + Sync>
            };
            let afuera = (p + q) * 0.5 - Vec3::new(c.x, (abajo + arriba) * 0.5, c.z);
            antorcha.push(lado_cono(vertice, p, q, afuera));
            antorcha.push(lado_cono(tapa, p, q, Vec3::new(0.0, 1.0, 0.0)));
        }
        // La llama: una esfera y dos lenguas de fuego encima.
        antorcha.push(Box::new(Sphere { center: tapa + Vec3::new(0.0, 0.12, 0.0), radius: 0.2, material: llama_naranja.clone() }));
        antorcha.push(Box::new(Sphere { center: tapa + Vec3::new(0.05, 0.32, 0.0), radius: 0.12, material: llama_naranja.clone() }));
        antorcha.push(Box::new(Sphere { center: tapa + Vec3::new(-0.03, 0.46, 0.02), radius: 0.07, material: llama_naranja.clone() }));
        objetos.push(Box::new(GrupoAcotado::estatico(antorcha)));
    }

    // ---- LA LLUVIA DE BRILLOS ----
    // En el juego los brillos no estan solo en la pared: llenan el aire y
    // caen sobre la fuente entera. Aca son ciento cincuenta gotas de luz que bajan
    // desde arriba del techo abierto hasta el agua, cada una por su columna,
    // a su velocidad, titilando. Cada gota va en su propio grupo, asi el
    // arbol (que se rearma en cada cuadro) las descarta de a una.
    // Transparente: la gota SUMA su luz a lo que hay detras. Opaca, una gota
    // tenue de dia se veia como un punto negro contra el cielo claro.
    let gota = Material::new([0.0, 0.0, 0.0, 1.0], 1.0, 1.0, Texture::Solid(Color::WHITE), Some(Color::BLACK));
    let lluvia = (0..GOTAS)
        .map(|_| {
            let i = objetos.len();
            // La gota y su estela: un trazo corto por encima, que es lo que la
            // hace leerse como algo que CAE y no como una mota quieta. La
            // estela es un cubo alineado a los ejes, finito y alto: las gotas
            // caen a plomo, y es la figura mas barata del trazador (con un
            // cilindro orientado la lluvia costaba el 9% del cuadro, casi todo
            // en las estelas).
            objetos.push(Box::new(GrupoAcotado::new(vec![
                Box::new(Sphere { center: Vec3::new(0.0, LLUVIA_TECHO, 0.0), radius: 0.0, material: gota.clone() })
                    as Box<dyn RayIntersect + Send + Sync>,
                Box::new(Cube::new_rect(Vec3::new(0.0, LLUVIA_TECHO, 0.0), 0.0, 0.0, 0.0, gota.clone())),
            ])));
            i
        })
        .collect();

    FuenteViva { lluvia }
}

impl FuenteViva {
    /// La cascada de brillos cae al ritmo del tema (un tramo por tiempo) y se
    /// enciende con el arpa: un piso que sigue a la energia y un destello en
    /// cada ataque, que se apaga en medio segundo.
    pub fn actualizar(&self, objetos: &mut [Box<dyn RayIntersect + Send + Sync>], p: &crate::sync::SceneParams) {
        use std::any::Any;
        let destello = p
            .ataques
            .iter()
            .filter_map(|&(_, t0)| {
                let edad = p.tiempo - t0;
                (0.0..0.6).contains(&edad).then(|| (1.0 - edad / 0.6).powi(2))
            })
            .fold(0.0f32, f32::max);
        // LA LLUVIA: cada gota cae por su columna, a su velocidad, y vuelve
        // a empezar arriba. Todo sale del numero de la gota y del segundo.
        // Mas gotas encendidas y mas brillo cuanto mas toca el arpa; de dia
        // casi no se ven, como estrellas.
        let noche = 1.0 - 0.75 * p.luz_del_dia;
        for (k, &i) in self.lluvia.iter().enumerate() {
            let Some(g) = objetos
                .get_mut(i)
                .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
            else {
                continue;
            };
            let h = |j: u32| hash(k as i32, j as i32, 97);
            let ang = h(1) * 2.0 * PI;
            // En un ANILLO alrededor del estrado, no encima: se lee como una
            // cortina de brillos que rodea la fuente, no le llueve encima al
            // hada, y la camara puede entrar al centro sin que una gota le
            // pase por delante como una bola blanca desenfocada.
            let r = LLUVIA_ADENTRO + (LLUVIA_RADIO - LLUVIA_ADENTRO) * h(2).sqrt();
            let velocidad = 0.9 + 0.8 * h(3);
            let caida = LLUVIA_TECHO - 0.1;
            let fase = (p.tiempo * velocidad / caida + h(4)).fract();
            let y = LLUVIA_TECHO - fase * caida;
            let x = ang.cos() * r + 0.15 * (p.tiempo * 0.7 + k as f32).sin();
            let z = ang.sin() * r;
            // Titila: cada gota prende y apaga a su ritmo, y el arpa la aviva.
            let titila = 0.5 + 0.5 * (p.tiempo * (3.0 + 4.0 * h(5)) + h(6) * 6.28).sin();
            let viva = (h(7) < 0.45 + 0.55 * p.energia_suave.clamp(0.0, 1.0)) as i32 as f32;
            let entra_sale = ((1.0 - fase) * 8.0).min(1.0) * (fase * 10.0).min(1.0);
            let k_luz = (0.55 + 0.75 * titila + destello * 0.6) * viva * entra_sale * noche;
            let (cr, cg, cb) = if h(8) < 0.6 { (225.0, 235.0, 255.0) } else if h(8) < 0.85 { (150.0, 190.0, 255.0) } else { (255.0, 150.0, 220.0) };
            let c = |v: f32, k: f32| (v * k).min(255.0) as u8;
            let aca = Vec3::new(x, y, z);
            for hijo in g.children_mut() {
                if let Some(e) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Sphere>() {
                    e.center = aca;
                    // Chica y titilante: un destello, no una bola. Grande y con la
                    // estela abajo se leia como un alfiler.
                    e.radius = if k_luz > 0.03 { 0.028 + 0.028 * titila } else { 0.0 };
                    e.material.emission_color = Some(Color::new(c(cr, k_luz), c(cg, k_luz), c(cb, k_luz), 255));
                } else if let Some(estela) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Cube>() {
                    let (ancho, largo) = if k_luz > 0.03 { (0.009, 0.30 + 0.25 * velocidad) } else { (0.0, 0.0) };
                    estela.min = aca - Vec3::new(ancho, 0.0, ancho);
                    estela.max = aca + Vec3::new(ancho, largo, ancho);
                    let k = k_luz * 0.40;
                    estela.material.emission_color = Some(Color::new(c(cr, k), c(cg, k), c(cb, k), 255));
                }
            }
            g.recalcular_caja(0.0);
        }

    }
}

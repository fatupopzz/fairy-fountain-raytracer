//! LA ISLA FLOTANTE: el diorama propiamente dicho.
//!
//! La fuente ya no esta en el fondo de una cueva sino arriba de un pedazo
//! de tierra suspendido en el cielo, como los dioramas de bloques: pasto
//! arriba, una capa de tierra y debajo roca que se va angostando hacia
//! abajo en escalones, como un iceberg dado vuelta. De los bordes caen dos
//! cascadas al vacio, de la panza cuelgan cristales encendidos y alrededor
//! flotan islotes mas chicos que dan escala y profundidad.
//!
//! LA FORMA SALE DE UNA GRILLA DE CELDAS, capa por capa. Cada capa incluye
//! las celdas que caen dentro de un radio que se achica con la
//! profundidad, deformado por ruido para que el borde no sea un circulo.
//! Despues, en cada fila de cada capa, las celdas contiguas se FUNDEN en
//! una sola caja: una capa de ochenta celdas termina siendo una docena de
//! cubos. Es lo que hace que la isla cueste lo que cuesta una docena de
//! cubos por capa y no un cubo por celda.

use crate::cube::Cube;
use crate::grupo_acotado::GrupoAcotado;
use crate::material::Material;
use crate::ray_intersect::RayIntersect;
use crate::sphere::Sphere;
use crate::sync::SceneParams;
use crate::texture::{campo_fbm, Texture, TextureImage};
use crate::triangle::Triangle;
use crate::vec3::Vec3;
use raylib::prelude::Color;
use std::any::Any;
use std::f32::consts::PI;
use std::sync::Arc;

/// La altura de la cara de arriba de la isla (el pasto). La plaza de marmol
/// esta apoyada encima, apenas mas alta.
pub const TECHO_ISLA: f32 = -0.42;

/// El lado de cada celda de la grilla.
const CELDA: f32 = 0.9;
/// Cuantas celdas por lado.
const CELDAS: i32 = 18;
/// El radio de la capa de arriba.
const RADIO: f32 = 7.6;
/// Cuantas capas de roca hay debajo de la de tierra, y su espesor.
const CAPAS: usize = 8;
const ESPESOR: f32 = 0.75;

/// Las cascadas: donde caen (x, z del borde), hacia que lado corre el agua
/// sobre el pasto, y el ancho.
const CASCADAS: [(f32, f32, f32, f32, f32); 2] = [
    // x, z, dir x, dir z, ancho
    (-7.1, 1.6, -1.0, 0.0, 0.9),
    (2.2, -7.1, 0.0, -1.0, 0.8),
];

/// Hasta donde caen las cascadas.
const CASCADA_FONDO: f32 = -13.0;

pub struct IslaViva {
    cascadas: Vec<usize>,
}

fn hash(x: i32, y: i32, k: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ k.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
}

/// Ruido suave de valor en el plano: bilineal entre hashes de una grilla
/// gruesa. Es lo que deforma el borde de cada capa.
fn ruido(x: f32, z: f32, escala: f32, k: u32) -> f32 {
    let (fx, fz) = (x / escala, z / escala);
    let (x0, z0) = (fx.floor(), fz.floor());
    let (tx, tz) = (fx - x0, fz - z0);
    let (sx, sz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let h = |a: f32, b: f32| hash(a as i32, b as i32, k);
    let arriba = h(x0, z0) + (h(x0 + 1.0, z0) - h(x0, z0)) * sx;
    let abajo = h(x0, z0 + 1.0) + (h(x0 + 1.0, z0 + 1.0) - h(x0, z0 + 1.0)) * sx;
    arriba + (abajo - arriba) * sz
}

/// Si la celda (i, j) esta en la capa `k` (0 es la de tierra, justo debajo
/// del pasto). El radio se achica con la profundidad siguiendo una raiz,
/// asi que las primeras capas son casi del ancho de la isla y las ultimas
/// se cierran en punta.
fn en_capa(i: i32, j: i32, k: usize) -> bool {
    let (x, z) = centro_celda(i, j);
    let r = (x * x + z * z).sqrt();
    let prof = k as f32 / CAPAS as f32;
    let radio = RADIO * (1.0 - prof).powf(0.75) * 0.97 + 0.4;
    let deforma = (ruido(x, z, 2.2, 11 + k as u32) - 0.5) * 2.4 + (ruido(x, z, 5.0, 3) - 0.5) * 1.6;
    // La capa de arriba no se deforma tanto: la plaza de 12 x 12 tiene que
    // apoyar entera sobre tierra.
    let deforma = if k == 0 { deforma.max(0.0) * 0.4 } else { deforma };
    r + deforma < radio || (k == 0 && x.abs() < 6.4 && z.abs() < 6.4)
}

fn centro_celda(i: i32, j: i32) -> (f32, f32) {
    (
        (i as f32 - (CELDAS as f32 - 1.0) / 2.0) * CELDA,
        (j as f32 - (CELDAS as f32 - 1.0) / 2.0) * CELDA,
    )
}

/// Las cajas de una capa: por cada fila, las celdas contiguas fundidas en
/// una sola caja.
fn cajas_de_capa(k: usize, y_arriba: f32, alto: f32, material: &Material, mosaico: f32) -> Vec<Box<dyn RayIntersect + Send + Sync>> {
    let mut cajas: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();
    for j in 0..CELDAS {
        let mut i = 0;
        while i < CELDAS {
            if !en_capa(i, j, k) {
                i += 1;
                continue;
            }
            let desde = i;
            while i < CELDAS && en_capa(i, j, k) {
                i += 1;
            }
            let (x0, z) = centro_celda(desde, j);
            let (x1, _) = centro_celda(i - 1, j);
            // Cada capa se mete un poquito bajo la de arriba para que no
            // quede una rendija de luz entre las dos.
            cajas.push(Box::new(
                Cube::new_rect(
                    Vec3::new((x0 + x1) * 0.5, y_arriba - alto * 0.5, z),
                    x1 - x0 + CELDA,
                    alto + 0.02,
                    CELDA,
                    material.clone(),
                )
                .con_mosaico(mosaico),
            ));
        }
    }
    cajas
}

/// El pasto: verde de Hyrule, con hojas mas claras y mas oscuras sueltas y
/// alguna florcita.
pub fn textura_pasto() -> TextureImage {
    let ruido = campo_fbm(128, 8, 4, 211);
    TextureImage::pintada(128, 128, move |u, v| {
        let (x, y) = ((u * 128.0) as i32, (v * 128.0) as i32);
        let n = ruido[(y as usize).min(127) * 128 + (x as usize).min(127)];
        let hoja = hash(x, y, 5);
        let k = 0.70 + 0.45 * n + (hoja - 0.5) * 0.25;
        let mut c = [0.26 * k, 0.55 * k, 0.17 * k];
        // Florcitas: una cada tanto, rosas y blancas.
        if hash(x / 3, y / 3, 9) > 0.992 {
            c = if hash(x / 3, y / 3, 10) > 0.5 { [1.0, 0.75, 0.9] } else { [1.0, 1.0, 0.85] };
        }
        c
    })
}

/// La tierra: marron con piedritas.
fn textura_tierra() -> TextureImage {
    let ruido = campo_fbm(128, 6, 4, 223);
    TextureImage::pintada(128, 128, move |u, v| {
        let (x, y) = ((u * 128.0) as i32, (v * 128.0) as i32);
        let n = ruido[(y as usize).min(127) * 128 + (x as usize).min(127)];
        let k = 0.70 + 0.45 * n + (hash(x, y, 7) - 0.5) * 0.18;
        if hash(x / 4, y / 4, 13) > 0.94 {
            // Una piedrita gris.
            let g = 0.45 + 0.2 * hash(x / 4, y / 4, 14);
            return [g, g * 0.97, g * 0.95];
        }
        [0.58 * k, 0.40 * k, 0.22 * k]
    })
}

/// Arma la isla entera. `piedra` es el material de roca de la escena y
/// `agua` el de la piscina, del que salen las cascadas.
pub fn armar(
    objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>,
    piedra: &Material,
    agua: &Material,
) -> IslaViva {
    let img = |t: TextureImage| Texture::ImageTexture(Arc::new(t), Color::WHITE, (0.0, 0.0));
    let pasto = Material::new([1.0, 0.06, 0.0, 0.0], 10.0, 0.0, img(textura_pasto()), None);
    let tierra = Material::new([1.0, 0.04, 0.0, 0.0], 8.0, 0.0, img(textura_tierra()), None);

    // ---- EL PASTO Y LA TIERRA ----
    // La capa de arriba: una lamina fina de pasto sobre una gruesa de
    // tierra, con el mismo contorno. De costado se lee como un bloque de
    // pasto de Minecraft: la franja verde arriba y la tierra debajo.
    let mut arriba = cajas_de_capa(0, TECHO_ISLA, 0.22, &pasto, 0.6);
    arriba.extend(cajas_de_capa(0, TECHO_ISLA - 0.22, 0.9, &tierra, 0.6));
    objetos.push(Box::new(GrupoAcotado::estatico(arriba)));

    // ---- LA ROCA ----
    // Una capa por grupo: los rayos que miran la isla de costado cruzan dos
    // o tres, y cada una tiene una docena de cajas.
    let mut fondo_de_capa = Vec::new();
    for k in 1..=CAPAS {
        let y = TECHO_ISLA - 1.12 - (k - 1) as f32 * ESPESOR;
        let capa = cajas_de_capa(k, y, ESPESOR, piedra, 0.45);
        if capa.is_empty() {
            break;
        }
        objetos.push(Box::new(GrupoAcotado::estatico(capa)));
        fondo_de_capa.push((k, y - ESPESOR));
    }

    // ---- LOS CRISTALES COLGANTES ----
    // Puntas de cristal encendido que cuelgan de la panza de la isla, en
    // las celdas que sobresalen de la capa de abajo (si no, quedarian
    // metidas en la roca). Emisivos: no los ilumina nada, brillan solos, y
    // desde abajo son lo que se ve de la isla.
    const COLORES: [(u8, u8, u8); 3] = [(255, 90, 200), (90, 220, 255), (170, 110, 255)];
    let mut colgantes: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();
    for &(k, y) in &fondo_de_capa {
        for j in 0..CELDAS {
            for i in 0..CELDAS {
                if !en_capa(i, j, k) || en_capa(i, j, k + 1) || hash(i, j, 31 + k as u32) < 0.86 {
                    continue;
                }
                let (x, z) = centro_celda(i, j);
                let largo = 0.8 + hash(i, j, 41) * 1.6;
                let (r, g, b) = COLORES[(hash(i, j, 43) * 3.0) as usize % 3];
                let material = Material::new(
                    [0.4, 0.8, 0.0, 0.0],
                    60.0,
                    0.0,
                    Texture::Solid(Color::new(r, g, b, 255)),
                    Some(Color::new(r / 2, g / 2, b / 2, 255)),
                );
                colgantes.extend(punta(Vec3::new(x, y + 0.05, z), 0.22 + hash(i, j, 47) * 0.14, largo, &material));
            }
        }
    }
    if !colgantes.is_empty() {
        objetos.push(Box::new(GrupoAcotado::estatico(colgantes)));
    }

    // ---- LAS CASCADAS ----
    // Cada una: un arroyito que corre por el pasto desde el borde de la
    // plaza, y el chorro que cae por el costado de la isla hasta perderse
    // en las nubes. El chorro es una caja fina de AGUA de verdad (refracta
    // como el agua de la piscina, se ve el cielo torcido a traves) con la
    // textura corriendo hacia abajo en cada cuadro.
    let chorro = Material::new(
        [0.35, 0.5, 0.25, 0.45],
        90.0,
        1.33,
        match &agua.texture {
            Texture::ImageTexture(imagen, _, _) => {
                Texture::ImageTexture(imagen.clone(), Color::new(120, 210, 255, 255), (0.0, 0.0))
            }
            otra => otra.clone(),
        },
        Some(Color::new(20, 70, 95, 255)),
    );
    let mut cascadas = Vec::new();
    for &(x, z, dx, dz, ancho) in &CASCADAS {
        let (sx, sz) = if dx != 0.0 { (0.14, ancho) } else { (ancho, 0.14) };
        // El borde: se camina desde el centro de la plaza hacia afuera
        // hasta salirse de la capa de arriba.
        let mut borde = 6.0f32;
        while borde < 9.0 {
            let (px, pz) = if dx != 0.0 { (dx * borde, z) } else { (x, dz * borde) };
            let i = ((px / CELDA) + (CELDAS as f32 - 1.0) / 2.0).round() as i32;
            let j = ((pz / CELDA) + (CELDAS as f32 - 1.0) / 2.0).round() as i32;
            if !en_capa(i, j, 0) {
                break;
            }
            borde += 0.1;
        }
        let borde = borde - 0.02;
        let caida_x = if dx != 0.0 { dx * (borde + 0.07) } else { x };
        let caida_z = if dz != 0.0 { dz * (borde + 0.07) } else { z };
        let alto = TECHO_ISLA + 0.03 - CASCADA_FONDO;
        let arroyo_largo = borde - 5.9;
        let (ax, az) = if dx != 0.0 {
            (dx * (5.9 + arroyo_largo * 0.5), z)
        } else {
            (x, dz * (5.9 + arroyo_largo * 0.5))
        };
        let (lx, lz) = if dx != 0.0 { (arroyo_largo, ancho) } else { (ancho, arroyo_largo) };
        let piezas: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
            Box::new(
                Cube::new_rect(Vec3::new(caida_x, TECHO_ISLA + 0.03 - alto * 0.5, caida_z), sx, alto, sz, chorro.clone())
                    .con_mosaico(0.5),
            ),
            Box::new(
                Cube::new_rect(Vec3::new(ax, TECHO_ISLA + 0.015, az), lx, 0.05, lz, chorro.clone()).con_mosaico(0.5),
            ),
        ];
        cascadas.push(objetos.len());
        objetos.push(Box::new(GrupoAcotado::new(piezas)));
    }

    // ---- LOS ISLOTES ----
    // Pedazos de tierra mas chicos flotando lejos, a distintas alturas. Son
    // lo que le da escala a la isla grande: sin nada alrededor, una isla
    // flotante en un cielo vacio no se sabe de que tamano es.
    for (n, &(angulo, distancia, altura, tam)) in [
        (0.6f32, 21.0f32, 1.5f32, 1.6f32),
        (2.3, 24.0, -2.5, 2.2),
        (3.6, 19.0, 3.2, 1.1),
        (4.4, 26.0, -0.5, 1.8),
        (5.5, 22.0, -4.0, 1.3),
    ]
    .iter()
    .enumerate()
    {
        let c = Vec3::new(angulo.cos() * distancia, altura, angulo.sin() * distancia);
        let mut islote: Vec<Box<dyn RayIntersect + Send + Sync>> = vec![
            Box::new(Cube::new_rect(c, tam * 2.0, 0.18, tam * 1.8, pasto.clone()).con_mosaico(0.6)),
            Box::new(
                Cube::new_rect(c - Vec3::new(0.0, 0.5, 0.0), tam * 1.9, 0.85, tam * 1.7, tierra.clone())
                    .con_mosaico(0.6),
            ),
        ];
        let mut ancho = tam * 1.6;
        let mut y = c.y - 0.9;
        let mut k = 0;
        while ancho > 0.3 {
            let corrido = Vec3::new((hash(n as i32, k, 3) - 0.5) * 0.4, 0.0, (hash(n as i32, k, 4) - 0.5) * 0.4);
            islote.push(Box::new(
                Cube::new_rect(Vec3::new(c.x, y - 0.35, c.z) + corrido, ancho, 0.7, ancho * 0.9, piedra.clone())
                    .con_mosaico(0.45),
            ));
            ancho *= 0.62;
            y -= 0.7;
            k += 1;
        }
        // Un cristal encendido en algunos, del color de los de la plaza.
        if n % 2 == 0 {
            let (r, g, b) = COLORES[n % 3];
            let material = Material::new(
                [0.4, 0.8, 0.0, 0.0],
                60.0,
                0.0,
                Texture::Solid(Color::new(r, g, b, 255)),
                Some(Color::new(r / 2, g / 2, b / 2, 255)),
            );
            let base = c + Vec3::new(0.2, 0.09, -0.1);
            islote.extend(punta_arriba(base, 0.28, 1.2, &material));
            islote.push(Box::new(Sphere {
                center: base + Vec3::new(0.0, 0.5, 0.0),
                radius: 0.09,
                material: material.clone(),
            }));
        }
        objetos.push(Box::new(GrupoAcotado::estatico(islote)));
    }

    IslaViva { cascadas }
}

/// Una punta de cristal que CUELGA: base cuadrada arriba en `techo`,
/// vertice abajo.
fn punta(techo: Vec3, lado: f32, largo: f32, material: &Material) -> Vec<Box<dyn RayIntersect + Send + Sync>> {
    piramide(techo, lado, -largo, material)
}

fn punta_arriba(base: Vec3, lado: f32, largo: f32, material: &Material) -> Vec<Box<dyn RayIntersect + Send + Sync>> {
    piramide(base, lado, largo, material)
}

/// Cuatro caras triangulares de una piramide de base cuadrada (girada 45
/// grados, que se lee mas cristal) con el vertice a `largo` de la base.
fn piramide(base: Vec3, lado: f32, largo: f32, material: &Material) -> Vec<Box<dyn RayIntersect + Send + Sync>> {
    let h = lado * 0.5;
    let apice = base + Vec3::new(0.0, largo, 0.0);
    let esquinas: Vec<Vec3> = (0..4)
        .map(|i| {
            let a = i as f32 * PI / 2.0;
            base + Vec3::new(a.cos() * h * 1.41, 0.0, a.sin() * h * 1.41)
        })
        .collect();
    (0..4)
        .map(|i| {
            let (p, q) = (esquinas[i], esquinas[(i + 1) % 4]);
            // El orden de los vertices deja la normal hacia afuera.
            let (b, c) = if largo < 0.0 { (q, p) } else { (p, q) };
            Box::new(Triangle {
                a: apice,
                b: c,
                c: b,
                uv_a: None,
                uv_b: None,
                uv_c: None,
                material: material.clone(),
            }) as Box<dyn RayIntersect + Send + Sync>
        })
        .collect()
}

impl IslaViva {
    /// Las cascadas corren: la textura se arrastra hacia abajo en el chorro
    /// y hacia afuera en el arroyo.
    pub fn actualizar(&self, objetos: &mut [Box<dyn RayIntersect + Send + Sync>], p: &SceneParams) {
        let corre = p.tiempo * 1.1;
        for &g in &self.cascadas {
            let Some(grupo) = objetos
                .get_mut(g)
                .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
            else {
                continue;
            };
            for (k, hijo) in grupo.children_mut().iter_mut().enumerate() {
                if let Some(c) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Cube>() {
                    if k == 0 {
                        c.material.texture.set_uv_offset(0.0, corre);
                    } else {
                        c.material.texture.set_uv_offset(corre * 0.3, corre * 0.3);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La plaza de marmol (12 x 12) apoya entera sobre la capa de arriba: no
    /// queda ningun borde de marmol volando sobre el vacio.
    #[test]
    fn la_plaza_apoya_entera() {
        for j in 0..CELDAS {
            for i in 0..CELDAS {
                let (x, z) = centro_celda(i, j);
                if x.abs() < 6.0 && z.abs() < 6.0 {
                    assert!(en_capa(i, j, 0), "hueco bajo la plaza en ({x}, {z})");
                }
            }
        }
    }

    /// Cada capa de roca es mas chica que la de arriba: la isla se angosta
    /// hacia abajo, que es lo que la hace una isla flotante y no una losa.
    #[test]
    fn la_isla_se_angosta_hacia_abajo() {
        let cuenta = |k: usize| (0..CELDAS).flat_map(|j| (0..CELDAS).map(move |i| (i, j))).filter(|&(i, j)| en_capa(i, j, k)).count();
        let tamanios: Vec<usize> = (0..CAPAS).map(cuenta).collect();
        assert!(tamanios.windows(2).all(|w| w[1] <= w[0] + 2), "{tamanios:?}");
        assert!(tamanios[CAPAS - 1] * 4 < tamanios[0], "{tamanios:?}");
    }
}

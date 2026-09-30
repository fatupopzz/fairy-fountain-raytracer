//! EL FUEGO DE DIN: el hechizo que el Hada Mayor le da a Link.
//!
//! En Ocarina of Time la Gran Hada no aparece por nada: bendice a Link y le
//! da un poder. El Fuego de Din es un cristal en forma de ROMBO; al usarlo,
//! Link se agacha, levanta los punos y golpea el piso con uno, y una cupula
//! de fuego se abre y se expande; al terminar, la cupula se deshace en ondas
//! hacia todos lados y se desvanece. Aca es el remate del climax: cuando el
//! hada se zambulle, en el primer tiempo fuerte siguiente Link estrena el
//! poder (ver `SyncData::hechizos`). La cupula no nace en Link sino en el
//! CENTRO de la fuente, donde se zambullo el hada, y crece hasta envolver
//! todo el estrado y a Link: el poder sale de la fuente.
//!
//! Esta en clave de sueno, no de explosion: la cupula es casi transparente y
//! de colores pastel (durazno, rosa, oro), se abre despacio, y lo que queda
//! al final son ondas de chispas. Tres tiempos:
//!
//!   1. EL ROMBO aparece sobre el cuenco, girando, y cae adentro justo
//!      cuando Link golpea el piso (`GOLPE`);
//!   2. se abre LA CUPULA: dos esferas translucidas emisivas, una adentro
//!      de la otra, que dejan ver la fuente a traves;
//!   3. al final, LAS ONDAS: un anillo de chispas que sale en todas las
//!      direcciones desde el centro, y se apagan.
//!
//! Y una luz durazno, suave, que tine la fuente mientras dura.

use crate::grupo_acotado::GrupoAcotado;
use crate::light::Light;
use crate::material::Material;
use crate::ray_intersect::RayIntersect;
use crate::sphere::Sphere;
use crate::sync::SceneParams;
use crate::texture::Texture;
use crate::triangle::Triangle;
use crate::vec3::{cross, dot, Vec3};
use raylib::prelude::Color;
use std::any::Any;
use std::f32::consts::PI;

/// Cuanto dura el hechizo, en segundos.
const VIDA: f32 = 3.4;
/// Cuanto dura el rombo antes de que Link golpee el piso.
pub const GOLPE: f32 = 0.55;
/// Hasta donde llega la cupula: pasa a Link (que esta a 3 m del centro).
const RADIO: f32 = 4.6;
/// Cuantas chispas hay en las ondas.
const CHISPAS: usize = 40;

pub struct FuegoDeDin {
    cupula: usize,
    rombo: usize,
    ondas: usize,
    luz: usize,
}

fn centro() -> Vec3 {
    Vec3::new(0.0, crate::fuente::CUENCO_Y + 0.25, 0.0)
}

fn velo(transparencia: f32) -> Material {
    Material::new([0.0, 0.0, 0.0, transparencia], 1.0, 1.0, Texture::Solid(Color::WHITE), Some(Color::BLACK))
}

fn suave(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub fn armar(objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>, luces: &mut Vec<Light>) -> FuegoDeDin {
    let cupula = objetos.len();
    objetos.push(Box::new(GrupoAcotado::new(vec![
        Box::new(Sphere { center: centro(), radius: 0.0, material: velo(0.94) }) as Box<dyn RayIntersect + Send + Sync>,
        Box::new(Sphere { center: centro(), radius: 0.0, material: velo(0.90) }),
    ])));

    // El rombo: un octaedro de ocho caras triangulares.
    let rombo = objetos.len();
    let cara = Material::new([0.6, 0.9, 0.3, 0.0], 120.0, 0.0, Texture::Solid(Color::new(255, 150, 170, 255)), Some(Color::BLACK));
    objetos.push(Box::new(GrupoAcotado::new(
        (0..8)
            .map(|_| {
                Box::new(Triangle {
                    a: centro(),
                    b: centro(),
                    c: centro(),
                    uv_a: None,
                    uv_b: None,
                    uv_c: None,
                    material: cara.clone(),
                }) as Box<dyn RayIntersect + Send + Sync>
            })
            .collect(),
    )));

    let ondas = objetos.len();
    objetos.push(Box::new(GrupoAcotado::new(
        (0..CHISPAS)
            .map(|_| Box::new(Sphere { center: centro(), radius: 0.0, material: velo(1.0) }) as Box<dyn RayIntersect + Send + Sync>)
            .collect(),
    )));

    let luz = luces.len();
    luces.push(Light::new(centro(), Color::new(255, 185, 165, 255), 0.0).con_alcance(4.5));
    FuegoDeDin { cupula, rombo, ondas, luz }
}

impl FuegoDeDin {
    pub fn actualizar(&self, objetos: &mut [Box<dyn RayIntersect + Send + Sync>], luces: &mut [Light], p: &SceneParams) {
        let e = p.hechizo;
        let vivo = (0.0..VIDA).contains(&e);
        let grupo = |objetos: &mut [Box<dyn RayIntersect + Send + Sync>], i: usize| {
            objetos.get_mut(i).and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>().map(|g| g as *mut GrupoAcotado))
        };

        // ---- 1. EL GOLPE ----
        // Aparece sobre el cuenco, sube girando, y cae adentro justo cuando
        // Link golpea el piso.
        let rombo = if vivo { suave(e / 0.15) * (1.0 - suave((e - GOLPE + 0.02) / 0.08)) } else { 0.0 };
        if let Some(g) = grupo(objetos, self.rombo) {
            let g = unsafe { &mut *g };
            let cae = suave((e - GOLPE + 0.15) / 0.15);
            let c = centro() + Vec3::new(0.0, (2.3 + 0.5 * e.min(GOLPE)) * (1.0 - cae), 0.0);
            let (alto, ancho) = (0.5 * rombo, 0.3 * rombo);
            let giro = e * 7.0;
            let ecuador: Vec<Vec3> = (0..4)
                .map(|k| {
                    let a = giro + k as f32 * PI / 2.0;
                    c + Vec3::new(a.cos() * ancho, 0.0, a.sin() * ancho)
                })
                .collect();
            let (arriba, abajo) = (c + Vec3::new(0.0, alto, 0.0), c - Vec3::new(0.0, alto, 0.0));
            for (k, hijo) in g.children_mut().iter_mut().enumerate() {
                if let Some(t) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Triangle>() {
                    let (a, b) = (ecuador[k % 4], ecuador[(k + 1) % 4]);
                    let punta = if k < 4 { arriba } else { abajo };
                    let n = cross(&(b - a), &(punta - a));
                    let (a, b) = if dot(&n, &((a + b) * 0.5 - c)) < 0.0 { (b, a) } else { (a, b) };
                    t.a = a;
                    t.b = b;
                    t.c = punta;
                    let k = rombo;
                    t.material.emission_color = Some(Color::new((255.0 * k) as u8, (120.0 * k) as u8, (150.0 * k) as u8, 255));
                }
            }
            g.recalcular_caja(0.0);
        }

        // ---- 2. LA CUPULA ----
        // Se abre despacio y frena (una onda que se ablanda), y el brillo
        // sube de golpe con el golpe de Link y despues se apaga lento.
        let d = e - GOLPE;
        let abierta = if vivo && d > 0.0 { 1.0 - (-d * 1.8).exp() } else { 0.0 };
        let brillo = if vivo && d > 0.0 { suave(d / 0.12) * (1.0 - suave((d - 0.4) / (VIDA - GOLPE - 0.9))) } else { 0.0 };
        if let Some(g) = grupo(objetos, self.cupula) {
            let g = unsafe { &mut *g };
            // (radio relativo, color): la de afuera durazno, la de adentro rosa.
            const CAPAS: [(f32, (f32, f32, f32)); 2] = [(1.0, (230.0, 150.0, 120.0)), (0.72, (230.0, 120.0, 190.0))];
            for (hijo, &(escala, (r, gg, b))) in g.children_mut().iter_mut().zip(CAPAS.iter()) {
                if let Some(esfera) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Sphere>() {
                    esfera.center = centro();
                    esfera.radius = RADIO * escala * abierta;
                    let k = brillo * 0.16;
                    esfera.material.emission_color = Some(Color::new((r * k) as u8, (gg * k) as u8, (b * k) as u8, 255));
                }
            }
            g.recalcular_caja(0.0);
        }

        // ---- 3. LAS ONDAS ----
        // Cuando la cupula se deshace, un anillo de chispas sale de Link en
        // todas las direcciones, apenas subiendo, y se apaga.
        let o = e - (GOLPE + 1.1);
        if let Some(g) = grupo(objetos, self.ondas) {
            let g = unsafe { &mut *g };
            for (k, hijo) in g.children_mut().iter_mut().enumerate() {
                if let Some(chispa) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<Sphere>() {
                    if vivo && o > 0.0 {
                        let a = k as f32 * 2.0 * PI / CHISPAS as f32 + (k % 3) as f32 * 0.07;
                        // Dos ondas: la segunda sale un poco despues y mas alta.
                        let (o, alto) = if k % 2 == 0 { (o, 0.0) } else { (o - 0.25, 0.7) };
                        let vuelo = RADIO * 0.7 + o.max(0.0) * 2.4;
                        let altura = 0.2 + alto + 0.3 * ((k * 7 % 5) as f32 / 4.0) + o.max(0.0) * 0.6;
                        chispa.center = centro() + Vec3::new(a.cos() * vuelo, altura, a.sin() * vuelo);
                        let vida = if o > 0.0 { (1.0 - o / (VIDA - GOLPE - 1.1)).max(0.0) } else { 0.0 };
                        chispa.radius = 0.1 * vida;
                        let tono = if k % 3 == 0 { (255.0, 225.0, 160.0) } else if k % 3 == 1 { (255.0, 170.0, 210.0) } else { (210.0, 190.0, 255.0) };
                        chispa.material.emission_color =
                            Some(Color::new((tono.0 * vida) as u8, (tono.1 * vida) as u8, (tono.2 * vida) as u8, 255));
                    } else {
                        chispa.radius = 0.0;
                    }
                }
            }
            g.recalcular_caja(0.0);
        }

        if let Some(luz) = luces.get_mut(self.luz) {
            luz.intensity = 2.4 * brillo + 1.0 * rombo;
        }
    }
}

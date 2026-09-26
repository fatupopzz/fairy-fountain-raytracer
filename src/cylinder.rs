use crate::material::Material;
use crate::ray_intersect::{Intersect, RayIntersect};
use crate::vec3::{cross, dot, normalize, Vec3};
use std::f32::consts::PI;

/// Cilindro vertical (alineado al eje Y), con tapas.
/// `center` es el centro de la BASE, y crece hacia arriba `height`.
/// Sirve para las columnas de la escena.
pub struct Cylinder {
    pub center: Vec3,
    pub radius: f32,
    pub height: f32,
    /// Radio del hueco central. En 0.0 el cilindro es macizo; con un
    /// valor mayor queda un ANILLO, que es lo que hace falta para el
    /// borde de una fuente: si fuera macizo taparia la poza entera.
    pub inner_radius: f32,
    /// Cuantas unidades del mundo mide una repeticion de la textura.
    /// Las UV se calculan en tamanio real y no de 0 a 1, porque si no
    /// una sola copia se estiraria a lo largo de TODO el contorno: en
    /// un cilindro de radio 6 eso son 37 unidades. Con esto las
    /// baldosas salen del mismo tamanio en el techo y en las columnas.
    pub tile_size: f32,
    pub material: Material,
}

impl RayIntersect for Cylinder {
    /// Lo que brilla solo no tapa.
    fn puede_tapar(&self) -> bool {
        self.material.emission_color.is_none()
    }

    /// La caja del cilindro: el radio en X y Z, y su altura real en Y.
    /// La esfera de `bounds` tiene que cubrir la diagonal, asi que en una
    /// columna de radio 0.25 y 4.5 de alto sobra por todos lados.
    fn aabb(&self) -> Option<(Vec3, Vec3)> {
        let r = self.radius;
        Some((
            Vec3::new(self.center.x - r, self.center.y, self.center.z - r),
            Vec3::new(self.center.x + r, self.center.y + self.height, self.center.z + r),
        ))
    }

    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        let y_min = self.center.y;
        let y_max = self.center.y + self.height;

        // Se guarda el impacto mas cercano que se vaya encontrando.
        let mut best_t = f32::INFINITY;
        let mut best_normal = Vec3::zeros();
        let mut best_uv = (0.0, 0.0);

        // --- CUERPO CURVO ---
        // Visto desde arriba, un cilindro vertical es un circulo: se
        // resuelve la cuadratica igual que una esfera pero ignorando Y.
        let ox = origin.x - self.center.x;
        let oz = origin.z - self.center.z;

        let a = direction.x * direction.x + direction.z * direction.z;
        // Si a es ~0 el rayo sube o baja recto: no le pega al costado,
        // solo puede pegarle a las tapas.
        //
        // Un anillo tiene DOS paredes: la de afuera y la del hueco. Se
        // resuelve la misma cuadratica con cada radio. La normal sale
        // radial en los dos casos; para la pared interna apunta al lado
        // contrario, pero de eso ya se encarga el volteo del final.
        for wall_radius in [self.radius, self.inner_radius] {
            if wall_radius <= 0.0 || a <= 1e-6 {
                continue;
            }

            let b = 2.0 * (ox * direction.x + oz * direction.z);
            let c = ox * ox + oz * oz - wall_radius * wall_radius;

            let discriminant = b * b - 4.0 * a * c;

            if discriminant >= 0.0 {
                let sqrt_d = discriminant.sqrt();
                let t1 = (-b - sqrt_d) / (2.0 * a); // pared de enfrente
                let t2 = (-b + sqrt_d) / (2.0 * a); // pared de atras

                for t in [t1, t2] {
                    // El epsilon evita que el rayo se re-intersecte con
                    // la superficie de la que acaba de salir.
                    if t <= 1e-3 || t >= best_t {
                        continue;
                    }

                    // El circulo es infinito hacia arriba y abajo: hay que
                    // recortarlo a la altura real del cilindro.
                    let y = origin.y + direction.y * t;
                    if y < y_min || y > y_max {
                        continue;
                    }

                    let point = origin + direction * t;
                    // Normal radial: la misma que una esfera pero aplastada
                    // en Y, porque la pared no se inclina hacia arriba.
                    let normal = normalize(&Vec3::new(
                        point.x - self.center.x,
                        0.0,
                        point.z - self.center.z,
                    ));

                    // U da la vuelta al cilindro, V sube por la pared,
                    // las dos medidas en unidades del mundo.
                    let around = (0.5 + normal.z.atan2(normal.x) / (2.0 * PI))
                        * (2.0 * PI * wall_radius);
                    best_uv = (
                        around / self.tile_size,
                        (point.y - self.center.y) / self.tile_size,
                    );
                    best_t = t;
                    best_normal = normal;
                }
            }
        }

        // --- TAPAS ---
        // Dos planos horizontales, recortados al circulo de la base.
        if direction.y.abs() > 1e-6 {
            for (cap_y, cap_normal) in [
                (y_min, Vec3::new(0.0, -1.0, 0.0)), // tapa de abajo
                (y_max, Vec3::new(0.0, 1.0, 0.0)),  // tapa de arriba
            ] {
                let t = (cap_y - origin.y) / direction.y;

                if t <= 1e-3 || t >= best_t {
                    continue;
                }

                // Solo cuenta si cae en la corona de la tapa: dentro
                // del radio externo y fuera del hueco.
                let point = origin + direction * t;
                let dx = point.x - self.center.x;
                let dz = point.z - self.center.z;
                let dist2 = dx * dx + dz * dz;
                if dist2 > self.radius * self.radius
                    || dist2 < self.inner_radius * self.inner_radius
                {
                    continue;
                }

                // En las tapas el UV es el disco mismo, tambien en
                // unidades del mundo para que peguen con el cuerpo.
                best_uv = (dx / self.tile_size, dz / self.tile_size);
                best_t = t;
                best_normal = cap_normal;
            }
        }

        if best_t.is_finite() {
            let point = origin + direction * best_t;

            // Si el rayo viene desde ADENTRO del cilindro, la normal
            // apunta al lado equivocado y la cara saldria negra. Se
            // voltea para que siempre mire hacia el rayo. Vale igual
            // para el cuerpo y para las tapas.
            if dot(direction, &best_normal) > 0.0 {
                best_normal = -best_normal;
            }

            Intersect::new(
                point,
                best_normal,
                best_t,
                &self.material,
                best_uv.0,
                best_uv.1,
            )
        } else {
            Intersect::empty()
        }
    }

    /// `center` es el centro de la BASE, asi que la esfera se centra media
    /// altura mas arriba. El radio va del eje a un borde de la tapa: es la
    /// hipotenusa entre el radio del cilindro y media altura.
    ///
    /// El hueco del anillo no cambia nada: lo que se acota es el bulto de
    /// afuera, y el hueco esta adentro de el.
    fn bounds(&self) -> Option<(Vec3, f32)> {
        let half_height = self.height / 2.0;
        let center = Vec3::new(
            self.center.x,
            self.center.y + half_height,
            self.center.z,
        );

        Some((
            center,
            (self.radius * self.radius + half_height * half_height).sqrt(),
        ))
    }
}

/// Un `Cylinder` apuntando a donde uno quiera.
///
/// El de arriba es vertical y punto: toda su matematica asume que el eje es
/// Y. Reescribirla para un eje cualquiera seria duplicar el archivo entero
/// y arriesgarse a romper lo que ya anda (las columnas, la pared, el piso).
///
/// En vez de eso, aca no se resuelve NADA de geometria: se lleva el rayo al
/// marco donde el cilindro SI es vertical, se le pregunta al de siempre, y
/// se trae el resultado de vuelta al mundo. La base es ortonormal, o sea que
/// no estira ni encoge nada: la distancia del impacto vale igual en los dos
/// marcos y no hay que corregirla.
///
/// Es lo que hace posibles los haces de laser cruzados: cilindros finitos,
/// con sus dos tapas, inclinados cada uno a su angulo.
pub struct CilindroOrientado {
    /// El mismo cilindro de siempre, en su marco propio: base en el origen
    /// y creciendo hacia +Y.
    local: Cylinder,
    /// Donde cae la base en el mundo.
    base: Vec3,
    /// Los tres ejes del marco. `eje` es el largo del cilindro (el Y local);
    /// `u` y `w` son los dos costados.
    u: Vec3,
    eje: Vec3,
    w: Vec3,
    /// Si esta apagado, el rayo lo atraviesa como si no existiera.
    ///
    /// Hace falta porque un haz de laser APAGADO no es un haz negro: no es
    /// nada. Bajarle la emision a cero no alcanza, porque el cilindro sigue
    /// siendo geometria: con difuso cero y emision negra se dibuja como una
    /// barra NEGRA que ademas tapa lo que tiene detras, y en un escenario
    /// oscuro eso se ve peor que dejarlo prendido. Un haz es luz en el aire;
    /// cuando se apaga, el aire vuelve a estar vacio.
    visible: bool,
}

impl CilindroOrientado {
    /// De `desde` hasta `hasta`, con el radio y el material dados.
    ///
    /// El largo sale de los dos puntos, asi que mover una punta reajusta el
    /// haz solo: no hay una altura escrita aparte que pueda quedar vieja.
    pub fn nuevo(
        desde: Vec3,
        hasta: Vec3,
        radius: f32,
        tile_size: f32,
        material: Material,
    ) -> Self {
        let delta = hasta - desde;
        let largo = delta.magnitude().max(1e-4);
        let eje = delta / largo;

        // Un vector cualquiera que NO sea paralelo al eje, para sacar el
        // primer costado. Si el eje ya es casi Y se usa X, si no Y: asi el
        // producto cruz nunca sale degenerado.
        let referencia = if eje.y.abs() > 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };

        let u = normalize(&cross(&referencia, &eje));
        let w = cross(&eje, &u); // ya sale unitario: u y eje son unitarios y perpendiculares

        CilindroOrientado {
            local: Cylinder {
                center: Vec3::zeros(),
                radius,
                height: largo,
                inner_radius: 0.0,
                tile_size,
                material,
            },
            base: desde,
            u,
            eje,
            w,
            visible: true,
        }
    }

    /// Lo vuelve a poner entre dos puntos nuevos.
    ///
    /// Recalcula el largo y los tres ejes igual que `nuevo`, para poder
    /// moverlo en cada cuadro. Lo usan las estelas del arpa, que cruzan la
    /// escena: hacerlas con un cilindro que se recoloca en vez de con una
    /// fila de esferas las deja LISAS (una fila de esferas se lee como una
    /// oruga, por mas que se solapen: cada una tiene su silueta) y ademas
    /// sale mas barato, una primitiva en vez de diez.
    pub fn recolocar(&mut self, desde: Vec3, hasta: Vec3) {
        let delta = hasta - desde;
        let largo = delta.magnitude().max(1e-4);
        self.eje = delta / largo;
        let referencia = if self.eje.y.abs() > 0.9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        self.u = normalize(&cross(&referencia, &self.eje));
        self.w = cross(&self.eje, &self.u);
        self.base = desde;
        self.local.height = largo;
    }

    /// El radio, para afinar o engrosar el haz sin rehacerlo.
    pub fn set_radio(&mut self, radio: f32) {
        self.local.radius = radio;
    }

    /// El material, para poder tocarlo despues de armada la escena: los
    /// keyframes le suben y bajan la emision al haz en cada cuadro.
    pub fn material_mut(&mut self) -> &mut Material {
        &mut self.local.material
    }

    /// Prende y apaga el haz. Ver el campo `visible`.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Del mundo al marco del cilindro. Como la base es ortonormal, la
    /// inversa de la rotacion es proyectar sobre cada eje.
    fn a_local(&self, origin: &Vec3, direction: &Vec3) -> (Vec3, Vec3) {
        let d = origin - self.base;

        (
            Vec3::new(dot(&d, &self.u), dot(&d, &self.eje), dot(&d, &self.w)),
            Vec3::new(
                dot(direction, &self.u),
                dot(direction, &self.eje),
                dot(direction, &self.w),
            ),
        )
    }

    /// De vuelta al mundo: se recombina con los tres ejes.
    fn direccion_a_mundo(&self, v: &Vec3) -> Vec3 {
        self.u * v.x + self.eje * v.y + self.w * v.z
    }
}

impl RayIntersect for CilindroOrientado {
    fn ray_intersect<'a>(&'a self, origin: &Vec3, direction: &Vec3) -> Intersect<'a> {
        if !self.visible {
            return Intersect::empty();
        }

        let (local_origin, local_direction) = self.a_local(origin, direction);

        let mut hit = self.local.ray_intersect(&local_origin, &local_direction);
        if !hit.is_intersecting {
            return hit;
        }

        // La distancia y las UV valen igual en los dos marcos; el punto y la
        // normal no, esos hay que traerlos.
        hit.point = self.base + self.direccion_a_mundo(&hit.point);
        hit.normal = self.direccion_a_mundo(&hit.normal);
        hit
    }

    fn puede_tapar(&self) -> bool {
        self.local.puede_tapar()
    }

    fn occluded(&self, origin: &Vec3, direction: &Vec3, max_distance: f32) -> bool {
        // Un haz de laser es emisivo puro: no tapa la luz de nadie. Se corta
        // aca antes de transformar nada.
        if self.local.material.emission_color.is_some() {
            return false;
        }

        let (local_origin, local_direction) = self.a_local(origin, direction);
        self.local
            .occluded(&local_origin, &local_direction, max_distance)
    }

    fn bounds(&self) -> Option<(Vec3, f32)> {
        // El mismo cilindro del marco local, con su centro llevado al mundo.
        let (centro_local, radio) = self.local.bounds()?;
        Some((self.base + self.direccion_a_mundo(&centro_local), radio))
    }
}

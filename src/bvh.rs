//! Un BVH sobre los objetos de la escena: el arbol que evita probarlos
//! todos.
//!
//! Sin esto, cada rayo pregunta objeto por objeto, y con ocho luces eso son
//! treinta preguntas por impacto para el rayo primario mas treinta por cada
//! rayo de sombra. La mayoria son "no", y casi todas se podian haber
//! contestado de antemano mirando POR DONDE va el rayo.
//!
//! El arbol agrupa los objetos por cercania y le pone a cada grupo su caja.
//! Un rayo que no toca la caja de un nodo no puede tocar nada de lo que hay
//! adentro, asi que se salta el subarbol entero con un solo test de
//! rebanadas. De treinta objetos se pasa a unos ocho o diez tests de nodo.
//!
//! NO ES DUEÑO DE NADA: guarda INDICES dentro de la lista de objetos, que
//! sigue viviendo donde estaba. Hace falta que sea asi porque la animacion
//! busca las piezas por su indice (la hada numero tal, el agua, la
//! Trifuerza) y meterlas adentro del arbol rompería esas referencias.
//!
//! Tres cosas de la serie de Jacco Bikker sobre como construir un BVH, que
//! son las que hacen la diferencia entre un arbol que sirve y uno que no:
//!
//!   1. El corte se elige con la HEURISTICA DE AREA (SAH): entre todos los
//!      cortes posibles gana el que minimiza `n_izq * area_izq + n_der *
//!      area_der`, o sea el que deja los dos lados chicos y con pocos
//!      objetos. Partir por la mitad a ojo arma arboles bastante peores.
//!   2. RECORRIDO ORDENADO: de los dos hijos se visita primero el que el
//!      rayo toca ANTES. Como cada impacto acorta el rayo, cuando le toca
//!      el turno al segundo hijo muchas veces ya quedo demasiado lejos y
//!      se descarta sin mirarlo.
//!   3. PILA en vez de recursion, para poder decidir ese orden.
//!
//! La construccion es exhaustiva (prueba todos los cortes en los tres ejes)
//! porque esta escena tiene treinta objetos: son unas dos mil cuentas una
//! sola vez al arrancar. Con miles de objetos habria que repartirlos en
//! cajones, que es lo que hace la version rapida del mismo metodo.

use nalgebra_glm::Vec3;

/// Cuantos objetos, como maximo, entran en una hoja. Con menos que esto no
/// vale la pena partir: el test del nodo costaria mas que probar los dos.
///
/// Se probaron 1, 2 y 4 sobre esta escena y las tres dan lo mismo dentro
/// del ruido de la medicion (32 a 35 ms). Queda en 2 porque es el valor
/// que menos nodos arma para el mismo resultado.
const OBJETOS_POR_HOJA: usize = 2;

/// Profundidad maxima de la pila del recorrido. Con 64 alcanza para un
/// arbol de 2^64 hojas, o sea para siempre.
const PILA: usize = 64;

struct Nodo {
    min: Vec3,
    max: Vec3,
    /// En una hoja, donde arrancan sus objetos dentro de `orden`. En un
    /// nodo interno, el indice del hijo izquierdo (el derecho es el
    /// siguiente).
    inicio: u32,
    /// Cuantos objetos tiene, si es hoja. Cero quiere decir nodo interno.
    cuenta: u32,
}

pub struct Bvh {
    nodos: Vec<Nodo>,
    /// Los indices de objeto, reordenados para que los de cada hoja queden
    /// pegados. Es lo unico que el arbol reordena: la lista de objetos de
    /// verdad no se toca.
    orden: Vec<usize>,
    /// Los que no se pueden acotar (un plano infinito). Se prueban siempre,
    /// que es lo unico seguro. En esta escena esta vacio.
    siempre: Vec<usize>,
}

impl Bvh {
    /// Arma el arbol con los objetos que se le den: `cajas` trae, para cada
    /// uno, su indice en la lista de objetos y su caja.
    pub fn construir(cajas: &[(usize, Option<(Vec3, Vec3)>)]) -> Bvh {
        let mut orden = Vec::with_capacity(cajas.len());
        let mut siempre = Vec::new();
        let mut caja_de = Vec::with_capacity(cajas.len());

        for (indice, caja) in cajas {
            match caja {
                Some(c) => {
                    orden.push(*indice);
                    caja_de.push(*c);
                }
                None => siempre.push(*indice),
            }
        }

        let mut bvh = Bvh {
            nodos: Vec::new(),
            orden,
            siempre,
        };

        if !bvh.orden.is_empty() {
            // `caja_de` va en el mismo orden que `orden` y se reordena con
            // el, para no tener que ir a buscar la caja de cada objeto.
            let n = bvh.orden.len();
            bvh.nodos.push(Nodo {
                min: Vec3::zeros(),
                max: Vec3::zeros(),
                inicio: 0,
                cuenta: n as u32,
            });
            bvh.partir(0, &mut caja_de);
        }

        bvh
    }

    /// Caja que contiene a los objetos `[inicio, inicio + cuenta)`.
    fn caja_de_rango(cajas: &[(Vec3, Vec3)], inicio: usize, cuenta: usize) -> (Vec3, Vec3) {
        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for (a, b) in &cajas[inicio..inicio + cuenta] {
            for eje in 0..3 {
                min[eje] = min[eje].min(a[eje]);
                max[eje] = max[eje].max(b[eje]);
            }
        }
        (min, max)
    }

    /// Area de la superficie de una caja. Es la medida que usa la
    /// heuristica: la probabilidad de que un rayo cualquiera atraviese una
    /// caja es proporcional a su area, no a su volumen.
    fn area(min: Vec3, max: Vec3) -> f32 {
        let d = max - min;
        if d.x < 0.0 {
            return 0.0;
        }
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// Parte el nodo `i` en dos, si conviene, y sigue hacia abajo.
    fn partir(&mut self, i: usize, cajas: &mut Vec<(Vec3, Vec3)>) {
        let (inicio, cuenta) = (self.nodos[i].inicio as usize, self.nodos[i].cuenta as usize);
        let (min, max) = Self::caja_de_rango(cajas, inicio, cuenta);
        self.nodos[i].min = min;
        self.nodos[i].max = max;

        if cuenta <= OBJETOS_POR_HOJA {
            return;
        }

        // --- Elegir el corte por area (SAH) ---
        //
        // El costo de no partir es `cuenta * area`. Se prueban todos los
        // cortes posibles en los tres ejes, usando el centro de cada objeto
        // como posicion candidata, y gana el de menor costo. Si ninguno
        // mejora al de no partir, el nodo se queda como hoja.
        let mut mejor: Option<(usize, f32, f32)> = None; // (eje, posicion, costo)

        for eje in 0..3 {
            for k in inicio..inicio + cuenta {
                let corte = (cajas[k].0[eje] + cajas[k].1[eje]) * 0.5;

                let mut n_izq = 0usize;
                let mut min_i = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
                let mut max_i = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
                let mut n_der = 0usize;
                let mut min_d = min_i;
                let mut max_d = max_i;

                for (a, b) in &cajas[inicio..inicio + cuenta] {
                    let centro = (a[eje] + b[eje]) * 0.5;
                    let (n, mn, mx) = if centro < corte {
                        (&mut n_izq, &mut min_i, &mut max_i)
                    } else {
                        (&mut n_der, &mut min_d, &mut max_d)
                    };
                    *n += 1;
                    for e in 0..3 {
                        mn[e] = mn[e].min(a[e]);
                        mx[e] = mx[e].max(b[e]);
                    }
                }

                if n_izq == 0 || n_der == 0 {
                    continue;
                }

                let costo = n_izq as f32 * Self::area(min_i, max_i)
                    + n_der as f32 * Self::area(min_d, max_d);

                if mejor.is_none_or(|(_, _, c)| costo < c) {
                    mejor = Some((eje, corte, costo));
                }
            }
        }

        let Some((eje, corte, costo)) = mejor else {
            return;
        };
        if costo >= cuenta as f32 * Self::area(min, max) {
            return;
        }

        // --- Repartir en su lugar ---
        // Los objetos que van a la izquierda se empujan al principio del
        // rango, igual que el particionado de un quicksort. `orden` y
        // `cajas` se mueven juntos para que sigan cuadrando.
        let mut izq = inicio;
        let mut der = inicio + cuenta - 1;
        while izq <= der {
            let centro = (cajas[izq].0[eje] + cajas[izq].1[eje]) * 0.5;
            if centro < corte {
                izq += 1;
            } else {
                self.orden.swap(izq, der);
                cajas.swap(izq, der);
                if der == 0 {
                    break;
                }
                der -= 1;
            }
        }

        let cuenta_izq = izq - inicio;
        if cuenta_izq == 0 || cuenta_izq == cuenta {
            return;
        }

        let hijo = self.nodos.len() as u32;
        self.nodos.push(Nodo {
            min: Vec3::zeros(),
            max: Vec3::zeros(),
            inicio: inicio as u32,
            cuenta: cuenta_izq as u32,
        });
        self.nodos.push(Nodo {
            min: Vec3::zeros(),
            max: Vec3::zeros(),
            inicio: izq as u32,
            cuenta: (cuenta - cuenta_izq) as u32,
        });

        self.nodos[i].inicio = hijo;
        self.nodos[i].cuenta = 0;

        self.partir(hijo as usize, cajas);
        self.partir(hijo as usize + 1, cajas);
    }

    /// A que distancia entra el rayo a la caja del nodo, o `None` si no
    /// entra o si entra mas lejos que `limite`.
    ///
    /// `inv` es `1 / direccion`, calculado UNA vez por rayo: es el truco de
    /// siempre para sacar las divisiones del bucle interno.
    fn entra(nodo: &Nodo, origen: &Vec3, inv: &Vec3, limite: f32) -> Option<f32> {
        let mut t_entra = 0.0f32;
        let mut t_sale = limite;

        for eje in 0..3 {
            let mut a = (nodo.min[eje] - origen[eje]) * inv[eje];
            let mut b = (nodo.max[eje] - origen[eje]) * inv[eje];
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            if a > t_entra {
                t_entra = a;
            }
            if b < t_sale {
                t_sale = b;
            }
            if t_entra > t_sale {
                return None;
            }
        }

        Some(t_entra)
    }

    /// Recorre el arbol y le pasa a `probar` los objetos que el rayo puede
    /// llegar a tocar, de cerca hacia lejos.
    ///
    /// `probar` recibe el indice del objeto y el limite vigente, y devuelve
    /// el limite nuevo: para el rayo primario, la distancia del impacto mas
    /// cercano encontrado hasta ahora, que es lo que le permite al
    /// recorrido descartar los nodos que quedaron mas lejos; para un rayo
    /// de sombra, el limite no se mueve (interesa lo que haya en todo el
    /// camino, no lo mas cercano).
    ///
    /// Devolver un limite de CERO corta el recorrido en seco. Es para el
    /// rayo de sombra que ya se topo con algo opaco: la luz ya no llega,
    /// asi que lo que haya mas alla da igual. Una distancia de verdad
    /// nunca es cero (el origen de todo rayo se desplaza un epsilon de la
    /// superficie), asi que no hay ambiguedad.
    pub fn recorrer(
        &self,
        origen: &Vec3,
        direccion: &Vec3,
        mut limite: f32,
        mut probar: impl FnMut(usize, f32) -> f32,
    ) {
        for &i in &self.siempre {
            limite = probar(i, limite);
            if limite <= 0.0 {
                return;
            }
        }

        if self.nodos.is_empty() {
            return;
        }

        let inv = Vec3::new(
            1.0 / direccion.x,
            1.0 / direccion.y,
            1.0 / direccion.z,
        );

        let mut pila = [0usize; PILA];
        let mut altura = 0usize;
        let mut actual = 0usize;

        loop {
            let nodo = &self.nodos[actual];

            if nodo.cuenta > 0 {
                let desde = nodo.inicio as usize;
                for &i in &self.orden[desde..desde + nodo.cuenta as usize] {
                    limite = probar(i, limite);
                    if limite <= 0.0 {
                        return;
                    }
                }
            } else {
                let a = nodo.inicio as usize;
                let b = a + 1;
                let ta = Self::entra(&self.nodos[a], origen, &inv, limite);
                let tb = Self::entra(&self.nodos[b], origen, &inv, limite);

                // El mas cercano primero: el otro va a la pila y muchas
                // veces, para cuando le toque, ya quedo fuera del limite.
                let (primero, segundo) = match (ta, tb) {
                    (Some(da), Some(db)) => {
                        if da <= db {
                            (Some(a), Some(b))
                        } else {
                            (Some(b), Some(a))
                        }
                    }
                    (Some(_), None) => (Some(a), None),
                    (None, Some(_)) => (Some(b), None),
                    (None, None) => (None, None),
                };

                if let Some(p) = primero {
                    if let Some(s) = segundo {
                        if altura < PILA {
                            pila[altura] = s;
                            altura += 1;
                        }
                    }
                    actual = p;
                    continue;
                }
            }

            // Se acabo esta rama: se vuelve a la ultima bifurcacion.
            if altura == 0 {
                return;
            }
            altura -= 1;
            actual = pila[altura];

            // Puede haber quedado fuera del limite mientras tanto.
            if Self::entra(&self.nodos[actual], origen, &inv, limite).is_none() {
                // Se vacia hasta encontrar uno que siga sirviendo.
                loop {
                    if altura == 0 {
                        return;
                    }
                    altura -= 1;
                    actual = pila[altura];
                    if Self::entra(&self.nodos[actual], origen, &inv, limite).is_some() {
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cajas de lado 1 en fila sobre el eje X, en 0, 2, 4, ...
    fn fila(n: usize) -> Vec<(usize, Option<(Vec3, Vec3)>)> {
        (0..n)
            .map(|i| {
                let x = i as f32 * 2.0;
                (
                    i,
                    Some((Vec3::new(x - 0.5, -0.5, -0.5), Vec3::new(x + 0.5, 0.5, 0.5))),
                )
            })
            .collect()
    }

    /// El arbol no pierde objetos: un rayo que atraviesa la fila entera
    /// los visita a todos, y de cerca hacia lejos.
    ///
    /// El orden es por HOJA, no por objeto: adentro de una hoja se prueban
    /// como quedaron al repartirlos, porque ordenar dos o tres objetos
    /// costaria mas que probarlos. Por eso lo que se exige es que ninguno
    /// aparezca lejos de su lugar, no que la lista salga perfectamente
    /// ordenada.
    #[test]
    fn los_visita_a_todos_de_cerca_a_lejos() {
        let bvh = Bvh::construir(&fila(12));
        let mut vistos = Vec::new();
        bvh.recorrer(
            &Vec3::new(-10.0, 0.0, 0.0),
            &Vec3::new(1.0, 0.0, 0.0),
            f32::INFINITY,
            |i, l| {
                vistos.push(i);
                l
            },
        );

        let mut ordenados = vistos.clone();
        ordenados.sort();
        assert_eq!(ordenados, (0..12).collect::<Vec<_>>(), "perdio o repitio objetos");

        for (lugar, &i) in vistos.iter().enumerate() {
            let corrimiento = (lugar as i32 - i as i32).abs();
            assert!(
                corrimiento < OBJETOS_POR_HOJA as i32,
                "el objeto {i} salio en el lugar {lugar}: el recorrido no va de cerca a lejos"
            );
        }
    }

    /// Un rayo que pasa de largo por arriba no visita a ninguno.
    #[test]
    fn descarta_lo_que_no_toca() {
        let bvh = Bvh::construir(&fila(12));
        let mut vistos = 0;
        bvh.recorrer(
            &Vec3::new(-10.0, 5.0, 0.0),
            &Vec3::new(1.0, 0.0, 0.0),
            f32::INFINITY,
            |_, l| {
                vistos += 1;
                l
            },
        );
        assert_eq!(vistos, 0);
    }

    /// Acortando el limite con el primer impacto, el recorrido deja de
    /// mirar lo que quedo detras: es de lo que vive el recorrido ordenado.
    #[test]
    fn el_limite_poda_lo_que_quedo_atras() {
        let bvh = Bvh::construir(&fila(12));
        let mut vistos = 0;
        bvh.recorrer(
            &Vec3::new(-10.0, 0.0, 0.0),
            &Vec3::new(1.0, 0.0, 0.0),
            f32::INFINITY,
            |i, _| {
                vistos += 1;
                // El objeto i esta en x = 2i, o sea a 10 + 2i del origen.
                10.0 + 2.0 * i as f32
            },
        );
        assert!(vistos < 12, "visito {vistos} de 12 con el limite puesto");
    }

    /// Devolver cero corta el recorrido: es lo que hace un rayo de sombra
    /// en cuanto se topa con algo opaco.
    #[test]
    fn el_cero_corta_el_recorrido() {
        let bvh = Bvh::construir(&fila(12));
        let mut vistos = 0;
        bvh.recorrer(
            &Vec3::new(-10.0, 0.0, 0.0),
            &Vec3::new(1.0, 0.0, 0.0),
            f32::INFINITY,
            |_, _| {
                vistos += 1;
                0.0
            },
        );
        assert_eq!(vistos, 1, "tenia que parar en el primero");
    }

    /// Los que no se pueden acotar se prueban siempre, apunte donde apunte
    /// el rayo.
    #[test]
    fn lo_que_no_se_acota_se_prueba_siempre() {
        let mut cajas = fila(4);
        cajas.push((99, None));
        let bvh = Bvh::construir(&cajas);

        let mut vistos = Vec::new();
        bvh.recorrer(
            &Vec3::new(-10.0, 50.0, 0.0),
            &Vec3::new(0.0, 1.0, 0.0),
            f32::INFINITY,
            |i, l| {
                vistos.push(i);
                l
            },
        );
        assert_eq!(vistos, vec![99]);
    }
}

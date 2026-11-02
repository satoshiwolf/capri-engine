# Auditoria exhaustiva de capri-engine

## Metadata

| Campo | Valor |
| :--- | :--- |
| Fecha | 2026-11-01 |
| Commit base | `3eb003832581438c403143a61ede96b753292301` |
| Version revisada | `0.3.0` |
| Alcance | Codigo Rust, contratos WebSocket, FSM, fisica, render, FFI Win32, concurrencia, pruebas, documentacion y configuracion |
| Estrategia | Revision paralela de arquitectura, seguridad/concurrencia y contratos/calidad, seguida de consolidacion y verificacion local |
| Modificacion de codigo fuente | Ninguna |

## Resumen ejecutivo

El repositorio contiene 26 archivos Rust con un total de 5,435 lineas. No existe
actualmente ningun archivo Rust que supere las 1,000 lineas, pero
`src/render/pipeline.rs` esta en 949 lineas y queda a 51 lineas del umbral.
`src/render/animation.rs` tiene 815 lineas. Ambos requieren intervencion
estructural antes de recibir nuevas responsabilidades.

La base compila, pasa las 34 pruebas existentes, cumple el formato y pasa
`cargo clippy -- -D warnings`. Esa cobertura no detecta varios riesgos de
integracion: el gateway WebSocket puede quedar sin progreso despues de entrar
en el bucle sincrono principal, el hit-testing no cumple el contrato documentado
de `AGENTS.md`, los `HWND` no tienen liberacion RAII y la FSM puede recibir
transiciones incoherentes desde `main.rs`.

Decision recomendada:

1. Bloquear nuevas funcionalidades en `render/pipeline.rs`.
2. Dividir `pipeline.rs` y `animation.rs` conservando sus fachadas y
   reexportaciones publicas.
3. Corregir primero el aislamiento del runtime Tokio, la FSM y la politica de
   transparencia, porque afectan contratos funcionales e invariantes.
4. Incorporar pruebas de integracion y criterios de rendimiento medibles antes
   de declarar completados los objetivos de 60/120 FPS, 25 MB y 0.01 ms.

## Politica de tamano y decision

| Rango | Decision |
| :--- | :--- |
| Menos de 800 lineas | Sin intervencion obligatoria por tamano; revisar cohesion al agregar responsabilidades |
| 800 a 899 lineas | Requiere justificacion y plan de division documentado |
| 900 a 999 lineas | Bloquea nuevas funcionalidades hasta extraer una responsabilidad coherente |
| 1,000 lineas o mas | Incumple la puerta de calidad y requiere intervencion obligatoria |

El limite debe aplicarse por cohesion, no por fragmentar artificialmente el
codigo. Al dividir se deben mantener las rutas publicas usadas por `main.rs`,
`render/sprites.rs`, `render/pipeline.rs` y consumidores externos de la crate.

## Inventario prioritario

| Archivo | Lineas totales | Codigo antes de pruebas | Pruebas | Decision |
| :--- | ---: | ---: | ---: | :--- |
| `src/render/pipeline.rs` | 949 | 843 | 106 | Intervencion obligatoria antes de crecer |
| `src/render/animation.rs` | 815 | 706 | 109 | Plan de division requerido |
| `src/main.rs` | 585 | 585 | 0 | Reducir responsabilidades de orquestacion |
| `src/physics/engine.rs` | 581 | 519 | 62 | Encapsular FSM y transiciones |
| `src/render/window.rs` | 536 | 524 | 12 | Separar FFI, propiedad y message pump |
| `src/render/sprites.rs` | 428 | 328 | 100 | Mantener encapsulamiento del cache |
| `src/desktop/ledges.rs` | 203 | 183 | 20 | Mover frontera FFI o actualizar la regla |
| `src/physics/ballistics.rs` | 176 | 138 | 38 | Mantener constantes fisicas centralizadas |
| `src/physics/collision.rs` | 172 | 143 | 29 | Evitar API de frames no usada |
| `src/render/vfx.rs` | 169 | 169 | 0 | Agregar pruebas de ciclo de vida |
| `src/desktop/monitors.rs` | 120 | 120 | 0 | Mover frontera FFI o actualizar la regla |
| `src/gateway/server.rs` | 71 | 71 | 0 | Corregir errores, backpressure y pruebas |
| `src/gateway/protocol.rs` | 65 | 65 | 0 | Agregar pruebas de contrato JSON |

## Hallazgos criticos

### P0-01. El gateway WebSocket puede quedar inactivo por inanicion del runtime

Evidencia:

- `src/main.rs:43` usa `#[tokio::main]`, cuyo modo predeterminado es
  current-thread.
- `src/main.rs:53` lanza el gateway dentro de ese mismo runtime.
- Desde `src/main.rs:129` hasta `src/main.rs:413` se ejecuta un bucle sincrono
  que no devuelve control al scheduler Tokio.
- `src/gateway/server.rs:30` y `src/gateway/server.rs:33` dependen de futuros
  que necesitan ser sondeados por ese runtime.

Impacto:

El servidor puede alcanzar a enlazar el puerto durante la inicializacion y luego
dejar de aceptar conexiones, leer comandos o entregar telemetria al entrar en
el bucle visual. Las pruebas actuales no detectan el problema porque no
conectan un cliente con el bucle de render activo.

Intervencion requerida:

- Ejecutar Tokio en uno o mas hilos dedicados con runtime multi-thread.
- Mantener Win32 y render en el hilo principal.
- Usar canales acotados entre gateway y host.
- Agregar una prueba de integracion que conecte y reciba telemetria despues de
  iniciar el bucle.

Compatibilidad:

La API publica de `WsGatewayServer` puede mantenerse. Cambia la forma de
arranque, no el contrato JSON.

### P0-02. Hit-testing y transparencia incumplen reglas inviolables de AGENTS.md

Evidencia:

- `src/render/window.rs:303` invoca `SetLayeredWindowAttributes`, operacion
  expresamente prohibida por la regla de preservacion del click-through.
- `src/render/window.rs:162-170` responde `WM_NCHITTEST` leyendo solo un
  `AtomicBool`.
- La consulta real de `AlphaBitmask` ocurre antes, en el bucle de render de
  `src/main.rs:165`, y luego se publica como estado atomico.

Impacto:

El contrato de `AGENTS.md` exige que `WM_NCHITTEST` consulte directamente una
instantanea inmutable o atomica de la mascara en RAM. La implementacion evita
GPU readback y es rapida, pero usa una senal derivada y potencialmente stale
por hasta un frame. Tampoco acredita por si sola el objetivo end-to-end de
0.01 ms.

Intervencion requerida:

- Publicar una instantanea de solo lectura con transformacion y mascara
  consultables desde el procedimiento de ventana.
- Mantener la ventana de entrada separada.
- Eliminar el uso de `SetLayeredWindowAttributes` sin romper la interfaz publica
  del overlay.
- Agregar una prueba del contrato de `WM_NCHITTEST` y una medicion reproducible
  de latencia.

## Hallazgos altos

### P1-01. Los `HWND` no tienen propiedad RAII

Evidencia:

- `src/render/window.rs:242` crea el `HWND` del overlay.
- `src/render/window.rs:287` crea el `HWND` de entrada.
- `CompositionOverlay`, declarado en `src/render/window.rs:328`, no implementa
  `Drop` ni destruye las ventanas.

Impacto:

Si falla la creacion de la ventana auxiliar o de DirectComposition, los handles
creados antes del fallo permanecen hasta terminar el proceso. Esto incumple la
regla de RAII estricto para recursos Win32.

Intervencion requerida:

- Envolver cada `HWND` en un propietario RAII.
- Liberar en orden inverso al de creacion.
- Implementar rollback ante fallos parciales.
- Probar un fallo inyectado despues de crear cada handle.

### P1-02. Una repisa estrecha puede provocar un panic en fisica

Evidencia:

- `src/desktop/ledges.rs:165` acepta spans desde 40 px.
- `src/physics/engine.rs:261-263` ejecuta
  `clamp(left + 48.0, right - 48.0)`.
- Si el ancho es menor que 96 px, el limite inferior supera al superior y
  `f32::clamp` entra en panico.

Impacto:

Un span expuesto de entre 40 y 95 px puede cumplir las condiciones de
aterrizaje de `src/physics/collision.rs:14` y luego destruir el proceso durante
la fase de asentamiento.

Intervencion requerida:

- Validar que el span admite el ancho de apoyo antes de entrar en `Sit`.
- Si no lo admite, iniciar caida o seleccionar un margen seguro.
- Agregar pruebas con anchos de 40, 95, 96 y valores degenerados.

### P1-03. La FSM no controla sus transiciones

Evidencia:

- `PhysicsEngine` expone `stance` y varios componentes internos mutables en
  `src/physics/engine.rs:22-35`.
- `src/main.rs:214-233` asigna `physics.stance` directamente para comandos.
- `sleep` establece `LayDown` sin `seated_surface`; en el siguiente frame,
  `src/physics/engine.rs:251-256` detecta la falta de soporte y ejecuta
  `begin_fall`.
- Otros comandos pueden conservar `flight`, `skid`, `dazed` o
  `landing_surface` obsoletos.

Impacto:

La autoridad de la FSM esta dividida entre `main.rs` y `PhysicsEngine`. Se
permiten combinaciones de estado y componentes que el tipo no impide. El
comando `sleep` no mantiene el estado solicitado.

Intervencion requerida:

- Agregar metodos de transicion como `set_upside`, `ride_cursor`, `sleep`,
  `wake` y `reset_stand`.
- Centralizar en ellos la limpieza de `flight`, `skid`, `dazed`,
  `seated_surface` y `landing_surface`.
- Mantener temporalmente los campos publicos mediante deprecaciones si se
  planea volverlos privados.
- Probar cada comando durante multiples ticks.

### P1-04. La simulacion no usa tick fijo ni hilo dedicado

Evidencia:

- `src/main.rs:131` calcula `dt` variable desde el reloj del frame.
- `src/main.rs:270` pasa ese `dt` directamente a
  `step_with_surfaces`.
- Fisica, entrada Win32 y render comparten el mismo bucle.

Impacto:

Contradice la arquitectura declarada en `AGENTS.md`. Un stall de presentacion
altera el paso fisico y los resultados pueden diferir entre 60 y 120 Hz.

Intervencion requerida:

- Introducir un acumulador con pasos fisicos fijos.
- Separar simulacion de render mediante una instantanea de estado.
- Conservar el orden visual actual durante una migracion por etapas.
- Probar escenarios equivalentes a 60 y 120 Hz con el mismo tiempo simulado.

### P1-05. `unsafe` esta fuera de las fronteras permitidas

Evidencia:

- `src/desktop/ledges.rs:74`
- `src/desktop/ledges.rs:143`
- `src/desktop/monitors.rs:56`
- `src/desktop/monitors.rs:70`
- `src/desktop/monitors.rs:81`

`AGENTS.md` limita `unsafe` a `src/render/window.rs` y
`src/desktop/hooks.rs`. Los bloques revisados tienen comentarios `// SAFETY:`
correctos y no se detecto una violacion de memoria, pero la ubicacion incumple
la politica documental.

Intervencion requerida:

- Trasladar los adaptadores Win32 a una frontera permitida conservando
  `scan_ledges`, `enumerate_monitors` y `virtual_desktop_bounds` como API
  estable, o actualizar formalmente `AGENTS.md` si estos modulos deben ser
  fronteras autorizadas.

### P1-06. Los lints globales ocultan deuda y debilitan la seguridad FFI

Evidencia:

`src/lib.rs:1` suprime globalmente:

- `unsafe_op_in_unsafe_fn`
- `unused_imports`
- `unused_variables`

Impacto:

`cargo clippy -- -D warnings` no puede detectar esas clases de problemas. La
supresion global de un lint de seguridad FFI es especialmente riesgosa.

Intervencion requerida:

- Eliminar la supresion global.
- Corregir las advertencias resultantes.
- Si queda un caso justificado, aplicar un `allow` local y documentar el motivo.

### P1-07. No existen pruebas de los objetivos de rendimiento ni del ciclo real

Evidencia:

- Hay 34 pruebas unitarias de matematicas, geometria y mapeos.
- No hay pruebas de integracion en `tests/`.
- No hay benchmark de hit-testing, memoria, FPS, jitter ni handlers Win32.
- README declara 60/120 FPS, 25 MB y 0.01 ms sin una medicion reproducible.

Intervencion requerida:

- Agregar un benchmark de CPU para la consulta de mascara.
- Medir memoria residente del proceso y recursos GPU por separado.
- Registrar percentiles de frame time y jitter en 60 y 120 Hz.
- Automatizar una prueba de gateway con el loop activo.

## Hallazgos medios

### P2-01. Un cliente puede dejar de recibir telemetria tras `Lagged`

`src/gateway/server.rs:52` solo maneja `Ok(event)` de
`broadcast::Receiver::recv`. Si el receptor queda atrasado, el error no se
procesa y el cliente puede quedar escuchando solo mensajes entrantes.

Intervencion: manejar explicitamente `Lagged` y `Closed`; continuar el bucle tras
`Lagged` y cerrar ante `Closed`.

### P2-02. Un error transitorio de `accept` termina el servidor silenciosamente

`src/gateway/server.rs:33` usa `while let Ok(...)` y luego retorna `Ok(())` en
`src/gateway/server.rs:60`. Un error no queda observable para el host.

Intervencion: registrar el error, reintentar con backoff o propagarlo.

### P2-03. El canal de comandos es ilimitado y el consumidor lo drena una vez por frame

Evidencia:

- `src/main.rs:49`
- `src/gateway/server.rs:11`

Un proceso local o una pagina abierta puede inundar el canal. El enlace a
`127.0.0.1` reduce el alcance, pero no evita saturacion local.

Intervencion: canal acotado, limite de conexiones, limite de tamano de mensaje,
token efimero y validacion de `Origin`.

### P2-04. El trabajador de repisas no se une al cierre

`src/main.rs:94-100` conserva un `JoinHandle` descartado. La senal es
`IS_RUNNING`, pero no hay `join`, cancelacion explicita ni cierre coordinado.

Intervencion: worker RAII cancelable con canal acotado y espera acotada al
cierre.

### P2-05. La telemetria de impacto lateral y de techo usa velocidad posterior al rebote

En `src/physics/engine.rs:291-297`, `src/physics/engine.rs:304-310` y
`src/physics/engine.rs:317-323`, `flight.bounce` modifica la velocidad antes de
construir el evento. `speed` y la intensidad visual pueden representar la
velocidad de salida en vez de la de impacto.

Intervencion: capturar velocidad y norma antes de `bounce` y usar ese valor para
telemetria, squash y VFX. Agregar pruebas de conservacion de la velocidad de
impacto.

### P2-06. Parte del protocolo aceptado no tiene efecto observable

Evidencia:

- `IncomingCommand::Action` acepta `target`, pero solo se registra.
- `perch_on_window` aparece en la documentacion del protocolo, pero no esta
  implementado en el `match` de `src/main.rs:213-235`.
- `PointAt` registra `screen`, pero solo usa `x` e `y`.

Intervencion: implementar la semantica declarada o documentar explicitamente
los campos reservados. Agregar pruebas de cada variante.

### P2-07. No existe un error de dominio comun

Las fronteras retornan `Result<_, String>` en ventana, pipeline, sprites y
main. `AGENTS.md` recomienda `EngineError`.

Intervencion: introducir un error tipado con ventana, GPU, assets, protocolo y
fisica, conservando mensajes diagnosticos.

### P2-08. Los administradores exponen almacenamiento mutable

Evidencia:

- `PhysicsEngine`: `src/physics/engine.rs:22-35`.
- `SpriteManager`: `src/render/sprites.rs:151`.
- `AlphaBitmask`: `src/render/hittest.rs:8-10`.

La mutacion externa puede eludir FSM, LRU o validacion de tamano.

Intervencion: agregar getters de solo lectura y deprecar mutaciones publicas
antes de volverlas privadas.

### P2-09. Hay constantes fisicas y visuales fuera de `core/config.rs`

Ejemplos:

- `src/physics/engine.rs:122`, `173`, `186-187`, `291`, `304`, `317`, `328`,
  `374` y `376`.
- `src/physics/ballistics.rs:48`, `52`, `102`, `122` y `134`.
- `src/physics/collision.rs:86-88`, `118` y `125`.
- `src/physics/pendulum.rs:45`, `47`, `49-50`.
- `src/render/vfx.rs:38-39`, `84-97`, `108` y `111`.

No todos son constantes fisicas globales, pero varios afectan simulacion o
respuesta visual y contradicen la regla de centralizacion.

Intervencion: mover solo las constantes de dominio a `core/config.rs` y dejar
en los modulos las constantes puramente geometricas o de algoritmo con nombre
explicativo.

## Hallazgos bajos

### P3-01. La documentacion declara modulos inexistentes y una arquitectura distinta

`AGENTS.md` menciona `src/desktop/hooks.rs` y
`src/gateway/dispatcher.rs`, que no existen. Tambien declara un hilo de fisica
separado, mientras la simulacion corre en el bucle visual.

Intervencion: implementar la estructura declarada o actualizar la documentacion
con el estado real y una ruta de migracion.

### P3-02. Hay ramas y campos sin uso funcional evidente

- `FlightPhase::FloorSlide` se implementa en
  `src/physics/ballistics.rs:100` pero nunca se asigna.
- `smear_offset` y `hair_lean` de `PendulumState` se calculan pero no se
  consumen fuera del propio modulo.
- `SquashState::update` conserva una API basada en frames que no se usa en
  produccion.

Intervencion: confirmar si son API publica planificada. Si no, deprecar antes de
eliminar. Si si, integrarlas y probarlas.

### P3-03. Una prueba no cubre todas las posturas que afirma cubrir

`src/render/animation.rs:712` se llama
`every_stance_has_an_explicit_clip`, pero no verifica `Stance::Corner`.

Intervencion: agregar `Corner` y una prueba exhaustiva basada en una lista
canonica de posturas.

### P3-04. Hay ausencia de pruebas directas en gateway, protocolo, VFX y monitores

No se encontraron pruebas en:

- `src/gateway/server.rs`
- `src/gateway/protocol.rs`
- `src/render/vfx.rs`
- `src/desktop/monitors.rs`

Intervencion: agregar pruebas de serializacion, errores de servidor, ciclo de
vida de VFX y geometria de monitores adyacentes/no adyacentes.

## Fortalezas confirmadas

- `physics` no depende de render, gateway ni Win32.
- `desktop` no depende de la simulacion.
- `main.rs` funciona como adaptador entre los subsistemas.
- El motor no contiene llamadas a LLMs ni logica cognitiva.
- No hay `unwrap`, `expect` ni `panic!` en rutas de produccion; los encontrados
  estan en pruebas.
- No hay `device.poll` ni `map_async` en `WM_NCHITTEST`.
- Los bloques `unsafe` revisados tienen justificaciones `// SAFETY:`.
- No se detectaron emojis en la busqueda de archivos fuente y documentacion.
- Los recursos wgpu mantienen ownership y la textura queda viva junto a su
  bind group.
- Las formulas existentes tienen pruebas de independencia temporal o geometria
  en las areas principales.

## Intervencion obligatoria para `pipeline.rs`

`src/render/pipeline.rs` debe conservarse como fachada publica durante la
division. Los submodulos propuestos son:

| Modulo propuesto | Responsabilidad |
| :--- | :--- |
| `render/pipeline/backend.rs` | `Renderer`, superficie, dispositivo, pipelines y buffers |
| `render/pipeline/textures.rs` | Cache de texturas, bind groups y subida RGBA |
| `render/pipeline/geometry.rs` | Vertices de sprites, quads y transformacion a clip space |
| `render/pipeline/effects.rs` | Geometria de chispas, grietas, ondas y estrellas |

Secuencia segura:

1. Agregar pruebas de caracterizacion para `Renderer`, `SpriteInstance`,
   `SpriteDraw`, `sprite_vertices` y `effect_vertices`.
2. Extraer primero geometria pura sin cambiar firmas.
3. Extraer cache de texturas.
4. Extraer inicializacion GPU manteniendo `Renderer::new` como fachada.
5. Reexportar los tipos publicos desde `pipeline.rs` o `render/mod.rs`.
6. Ejecutar `cargo fmt --check`, clippy, check y test despues de cada extraccion.

## Intervencion obligatoria para `animation.rs`

Dividir en:

| Modulo propuesto | Responsabilidad |
| :--- | :--- |
| `render/animation/catalog.rs` | `ClipId`, `ClipSpec`, `ClipCatalog` y plantillas |
| `render/animation/controller.rs` | Acciones, prioridades, relojes, eventos y ambiente |

`animation.rs` debe seguir reexportando todos los tipos usados por
`main.rs`, `sprites.rs` y `pipeline.rs`. No se debe cambiar el nombre ni el
contrato publico durante la division.

## Reduccion recomendada de `main.rs`

Aunque `main.rs` no supera 1,000 lineas, concentra demasiadas fases. La
extraccion recomendada es:

- `command_dispatcher`: traduce `IncomingCommand` a transiciones de dominio.
- `telemetry`: traduce `EngineEvent` a `OutgoingEvent`.
- `render_coordinator`: seleccion, precarga y retencion de sprites.
- `engine_host`: organiza `handle_input`, `apply_commands`,
  `step_simulation`, `dispatch_visual_events` y `render_frame`.

`main` debe quedar como composition root y punto de arranque.

## Pruebas faltantes prioritarias

| Prioridad | Prueba requerida |
| :--- | :--- |
| P0 | Gateway conectado y funcionando con el bucle visual activo |
| P0 | `WM_NCHITTEST` consultando una instantanea real de mascara |
| P1 | Repisas de 40, 95 y 96 px |
| P1 | `sleep`, `hang_on_ceiling`, `hang_on_cursor`, `wake` y `reset_stand` durante varios ticks |
| P1 | Creacion fallida de input HWND y rollback de recursos |
| P1 | Fisica equivalente a 60 y 120 Hz con tick fijo |
| P2 | Cliente lento, canal lleno y `broadcast::Lagged` |
| P2 | Handshake con `Origin` no permitido, token ausente y mensaje sobredimensionado |
| P2 | Serializacion y compatibilidad de cada comando y evento JSON |
| P2 | Impactos laterales y de techo con velocidad previa al rebote |
| P3 | Todas las posturas, incluida `Corner` |
| P3 | Ciclo de vida y limites de VFX |
| P3 | Monitores adyacentes, separados, negativos y con distinta altura |

## Verificacion ejecutada

| Comando | Resultado |
| :--- | :--- |
| `cargo check` | Correcto |
| `cargo test` | Correcto: 34 pruebas, 0 fallos |
| `cargo clippy -- -D warnings` | Correcto |
| `cargo fmt --check` | Correcto |

No se ejecuto `cargo run --release` porque abre una aplicacion de escritorio
interactiva y no forma parte segura de una auditoria automatizada. No se midieron
FPS, memoria residente ni latencia end-to-end.

## Criterio de cierre recomendado

Antes de aceptar nuevas funcionalidades de render o animacion:

- `pipeline.rs` debe quedar por debajo de 900 lineas mediante extraccion
  cohesionada.
- `animation.rs` debe quedar con una sola responsabilidad principal.
- Deben existir pruebas de gateway activo y contrato de hit-testing.
- No deben quedar errores de runtime ignorados en gateway ni panics posibles en
  fisica.
- La FSM debe poseer todas las transiciones de estado.
- Cualquier cambio en `src/` debe incrementar `Cargo.toml` en PATCH, salvo que
  introduzca contrato nuevo o incompatible, en cuyo caso aplica MINOR o MAJOR.

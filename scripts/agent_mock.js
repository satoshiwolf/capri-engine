const WS_URL = 'ws://127.0.0.1:47823';

console.log(`[TEST CLIENT] Conectando a Capri Engine en ${WS_URL}...`);

const ws = new WebSocket(WS_URL);

ws.addEventListener('open', () => {
    console.log('[TEST CLIENT] ¡Conexión WebSocket establecida con Capri Engine!');

    setTimeout(() => {
        const cmd = {
            type: 'speak',
            text: '¡Capri Engine en Rust conectado y respondiendo, choom!',
            duration_ms: 3000
        };
        console.log('[TEST CLIENT] Enviando comando:', cmd);
        ws.send(JSON.stringify(cmd));
    }, 500);

    setTimeout(() => {
        const cmd = {
            type: 'emotion',
            state: 'happy'
        };
        console.log('[TEST CLIENT] Enviando comando:', cmd);
        ws.send(JSON.stringify(cmd));
    }, 1500);

    setTimeout(() => {
        const cmd = {
            type: 'action',
            action: 'hang_on_ceiling'
        };
        console.log('[TEST CLIENT] Enviando comando:', cmd);
        ws.send(JSON.stringify(cmd));
    }, 2500);

    setTimeout(() => {
        console.log('[TEST CLIENT] Prueba finalizada con éxito.');
        ws.close();
        process.exit(0);
    }, 4000);
});

ws.addEventListener('message', (event) => {
    try {
        const data = JSON.parse(event.data);
        console.log('[TEST CLIENT] Telemetría recibida desde Capri:', data);
    } catch {
        console.log('[TEST CLIENT] Mensaje raw:', event.data);
    }
});

ws.addEventListener('error', (err) => {
    console.error('[TEST CLIENT] Error de conexión:', err);
});

ws.addEventListener('close', () => {
    console.log('[TEST CLIENT] Conexión cerrada.');
});

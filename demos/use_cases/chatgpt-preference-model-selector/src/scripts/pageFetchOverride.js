// pageFetchOverride.js
(function() {
  const TAG = '[ModelSelector][Page]';
  console.log(`${TAG} installing fetch override`);

  window.archgwSettings = window.archgwSettings || { preferences: [], defaultModel: null };
  window.addEventListener('message', ev => {
    if (ev.source === window && ev.data?.type === 'PBMS_SETTINGS') {
      window.archgwSettings = ev.data.settings;
      console.log(`${TAG} got updated settings`, window.archgwSettings);
    }
  });

  const origFetch = window.fetch;
  window.fetch = async function(input, init = {}) {
    const urlString = typeof input === 'string' ? input : input.url;
    console.log(`${TAG} fetch →`, urlString);

    let pathname;
    try {
      pathname = new URL(urlString).pathname;
    } catch {
      pathname = urlString;
    }

    if (pathname === '/backend-api/conversation') {
      console.log(`${TAG} matched conversation → proxy via content script`);

      // patch metadata
      let body = {};
      try { body = JSON.parse(init.body); } catch {}
      body.metadata = {
        archgw_preference_config: window.archgwSettings.preferences
          .map(p => `- name: ${p.name}\n  model: ${p.model}\n  usage: ${p.usage}`)
          .join('\n')
      };
      init.body = JSON.stringify(body);

      // send only the serializable parts of `init`
      const safeInit = {
        method: init.method,
        headers: init.headers,
        body: init.body,
        credentials: init.credentials
      };

      // set up MessageChannel
      const { port1, port2 } = new MessageChannel();
      window.postMessage({
        type: 'ARCHGW_FETCH',
        url: 'http://localhost:12000/v1/chat/completions',
        init: safeInit
      }, '*', [port2]);

      // return streaming response
      return new Response(new ReadableStream({
        start(controller) {
          port1.onmessage = ({ data }) => {
            if (data.done) {
              controller.close();
              port1.close();
            } else {
              controller.enqueue(new Uint8Array(data.chunk));
            }
          };
        },
        cancel() {
          port1.close();
        }
      }), {
        headers: { 'Content-Type': 'text/event-stream' }
      });
    }

    return origFetch(input, init);
  };

  console.log(`${TAG} fetch override installed`);
})();

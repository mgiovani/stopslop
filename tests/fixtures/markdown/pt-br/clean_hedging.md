# Camada de Retentativas

O serviço tenta a operação até três vezes antes de desistir e registrar o erro no log. De fato, essa margem reduziu bastante o número de alertas falsos que a equipe recebia durante os picos de tráfego.

A nova versão do cliente é relativamente simples e praticamente não muda o comportamento observado pelos usuários finais. Ela também ficou basicamente mais previsível depois que o time ajustou o tempo de espera entre tentativas.

Certamente ainda existem casos em que a retentativa não resolve o problema, principalmente quando a falha vem do próprio banco de dados. Nesses casos, o serviço propaga o erro original em vez de mascará-lo com uma mensagem genérica.

Pode ser que a próxima versão adicione um limite configurável por chamada, mas isso ainda depende de testes em produção. Vale a pena testar cada mudança em um ambiente isolado antes de liberar para todos os clientes.

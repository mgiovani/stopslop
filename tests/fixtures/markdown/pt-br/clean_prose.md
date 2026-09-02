# Camada de Cache

O serviço mantém duas camadas de armazenamento em memória para reduzir a latência das consultas mais frequentes. A primeira camada guarda apenas as chaves acessadas nos últimos cinco minutos, enquanto a segunda guarda um conjunto maior com expiração de uma hora.

Quando uma chave não é encontrada na primeira camada, o sistema consulta a segunda antes de recorrer ao banco de dados principal. Essa estratégia reduziu o tempo médio de resposta de trezentos milissegundos para menos de quarenta, segundo os números coletados no último trimestre.

A equipe de infraestrutura acompanha o desempenho das duas camadas por meio de um painel que mostra a taxa de acerto de cada uma separadamente. Um alerta dispara sempre que a taxa de acerto da primeira camada cai abaixo de setenta por cento durante mais de dez minutos seguidos.

Esse limite foi ajustado depois de alguns testes em produção, porque um valor mais baixo deixava passar problemas sem que ninguém percebesse a tempo. Cada novo serviço que precisa de cache passa primeiro por uma revisão simples, na qual alguém explica por que os dados mudam pouco e por que vale a pena mantê-los perto da aplicação antes de qualquer código ser escrito.

# Notas de Manutenção

O time realmente testou cada cenário antes de liberar a atualização para produção.

Basicamente, o novo índice reduz o tempo de resposta em consultas complexas.

Quando se trata de rollback, a equipe sempre revisa os logs com cuidado antes de agir.

Como vimos no incidente da semana passada, monitorar as métricas certas faz toda a diferença.

Fundamentalmente, o cache local resolve a maior parte das consultas repetidas.

Na era do armazenamento em nuvem, times pequenos ainda mantêm backups locais por segurança.

# Notas sobre Cache

Vale ressaltar que o cache reduz bastante a carga no banco de dados.

Em tese, o cache deveria ficar habilitado para todos os serviços de produção mais críticos.

De um modo geral, monitore a taxa de acerto antes de qualquer ajuste na política de expiração.

Um início frio pode potencialmente atrasar as primeiras requisições logo após o novo deploy.

O time também percebeu que o serviço talvez possivelmente precise de mais réplicas na próxima fase.

<!-- expect-line: 3 SLOP015 -->
<!-- expect-line: 9 SLOP015 -->
<!-- expect-line: 11 SLOP015 -->

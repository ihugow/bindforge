# Bloqueio de sessão e suspensão

No modo `--kde`, o daemon acompanha a sessão gráfica pelo barramento de sistema
do `systemd-logind`. Ele pausa o mouse quando a sessão é bloqueada, fica inativa
ou o computador se prepara para suspender. A saída libera as ações mantidas,
é destruída e a captura física é liberada pelo mesmo caminho da pausa manual.

Ao desbloquear, reativar a sessão ou retornar da suspensão, o remapeamento
permanece pausado. O painel **Início → Remapeamento do mouse** informa o motivo.
Troque para uma janela se necessário para renovar o foco e use **Ativar
remapeamento**. A interface e o plano são validados novamente; se o identificador
do mouse mudou, atualize o inventário e refaça o vínculo em Dispositivos.

## Monitor e recuperação

- O monitor resolve a sessão por PID, com fallback para a sessão gráfica `auto`
  do chamador quando o processo está no gerenciador de serviços do usuário.
- Acompanha `LockedHint`, `Active` e `PrepareForSleep`. Assina os sinais antes
  da leitura inicial, fixa o dono único de logind e valida origem, caminho e
  interface. Há uma conferência de propriedades e dono a cada 500 ms, além do
  tempo das consultas D-Bus. Uma transição breve de bloqueio não é descartada
  somente porque o snapshot posterior já está desbloqueado.
- Um inibidor `sleep` em modo `delay` dá tempo para encerrar o remapeador antes
  de suspender. O descritor é fechado depois da liberação da entrada; no retorno,
  um novo descritor é adquirido antes de permitir ativação. Ele não impede a
  suspensão de forma permanente: o atraso máximo é definido pelo logind.
- Falha do barramento, troca do dono de logind, sessão indisponível ou recusa do
  inibidor pausam a captura e impedem ativação. O monitor tenta reconectar a cada
  segundo. Reconectar também exige ativação explícita; foco e recarga de perfis
  não removem uma restrição de sessão.
- As restrições e `StartMouse` usam o mesmo mutex. Uma ativação concorrente é
  recusada ou interrompida assim que o evento é processado.

O daemon não bloqueia/desbloqueia a sessão, não suspende o computador e não
instala serviços ou regras de permissão. A UI continua recebendo mudanças pelo
canal de estado existente, sem um novo polling de sessão no frontend.

Referência do protocolo: [systemd-logind D-Bus API](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html).

## Validação — 23 de outubro de 2026

Testes em barramento privado cobrem início bloqueado, desbloqueio, pausa manual
sem remoção da restrição, foco durante bloqueio, suspensão/retorno, inatividade,
transição breve de bloqueio, rejeição de sinais de outro processo, liberação e
renovação do descritor de atraso, troca de dono, ausência do serviço, recusa do
inibidor (inclusive no retorno), recuperação e desconexão do barramento.
Esses testes não bloqueiam nem suspendem a sessão real e não capturam hardware.

Os testes do motor já cobrem a liberação das ações mantidas. O self-test virtual
no kernel continua dependendo do acesso aos nós virtuais de teste, que apresentou
EACCES na rodada anterior.

Na sessão real, o logind reconheceu a sessão gráfica e ListInhibitors confirmou
o registro sleep/delay do BindForge, executado como usuário comum. O usuário
confirmou bloqueio/desbloqueio com um lateral mantido pressionado, sem teclas
presas, e pausa até nova ativação. Confirmou também suspensão/retorno com
remapeamento pausado e reativação correta.

Um observador temporário consultou somente estado IPC e propriedades do logind,
sem ler eventos físicos. Registro de 23 de outubro de 2026, horário de São Paulo:

| Horário | Observação |
| --- | --- |
| 14:45:22 | Mouse ativo, sessão desbloqueada |
| 14:45:34 | LockedHint=true; captura pausada por bloqueio |
| 14:45:43 | LockedHint=false; captura continua pausada, aguardando ativação |
| 14:46:00 | Captura ativa novamente |
| 14:46:29 | PreparingForSleep=true; captura pausada para suspensão |
| 14:46:39 | Retorno da suspensão, tela bloqueada e captura pausada |
| 14:46:48 | Sessão desbloqueada, captura continua pausada |
| 14:47:09 | Captura ativa novamente |

A consulta final confirmou a ponte KWin conectada e o inibidor sleep/delay
renovado. A suíte passou com 64 testes Rust e 26 testes do frontend; build do
daemon e TypeScript/Vite aprovados. A validação física foi feita com o Logitech
G PRO Wireless; outros modelos e outros ambientes gráficos não foram validados.

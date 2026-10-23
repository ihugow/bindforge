# Validação do mouse físico — 23 de outubro de 2026

Repositório: `/home/hugostoso/Dev/Projects/Personal/bindforge/`.
Dispositivo: Logitech G PRO Wireless, interface composta USB.

## Evidências iniciais

O daemon já estava em execução como usuário comum (UID 1000), com o mouse
cadastrado ativo. `Input1.GetInputStatus` retornou `active=true`, sem mensagem
de erro; `Status1.GetStatusWithHistory` retornou `simulation=false`, perfil
`default` e foco identificado. `Integration1.GetIntegration` confirmou a ponte
KWin conectada e recebendo foco. Os descritores do processo confirmaram a abertura
do mouse físico e de `/dev/uinput`. A sessão existente foi preservada.

O perfil Padrão associa o botão 8 a `Win+H` e o botão 9 a `Win`. Os perfis
Arc Raiders e Marvel Rivals estão habilitados e sem ações de mouse, permitindo
comparar o remapeamento do Padrão com o comportamento original nos jogos.

## Resultados

| Verificação | Resultado | Evidência |
| --- | --- | --- |
| Captura física sem executar o daemon como root | Confirmada | UID do processo, estado IPC e descritores abertos |
| Acionar e soltar os dois botões laterais no Padrão | Passou | Usuário confirmou os dois atalhos funcionando, sem teclas presas |
| Pausar e reativar pela interface | Passou | Usuário confirmou; sinais IPC mostraram `simulation=true` na revisão 59 e `false` na revisão 66 |
| Trocar entre Padrão e perfil de aplicativo | Passou | Usuário confirmou mudança do perfil e das ações; sinais IPC mostraram Padrão → Arc Raiders (revisão 81) → Padrão (82), mantendo `simulation=false` |
| Trocar de perfil enquanto um botão está pressionado | Passou | Usuário confirmou troca, soltura e nova ação sem teclas ou botões presos |
| Desconectar o mouse durante a captura | Passou | Usuário confirmou pausa e reativação após reconectar |
| Encerrar o daemon durante a captura | Passou | Usuário confirmou comportamento original; processo/serviço anterior encerraram e nenhuma saída virtual permaneceu no sysfs |
| Reiniciar o daemon | Passou | Usuário confirmou reconexão e inicialização pausada; consulta posterior encontrou novo processo e ponte KWin conectada |
| Bloqueio de sessão e suspensão | Passou na etapa posterior | [Validação específica](session-lifecycle.md), incluindo lateral mantido durante bloqueio e ativação explícita ao retornar |

## Verificações automatizadas desta rodada

- `cargo test --workspace --all-features --locked --offline`: 62 testes passaram.
- `npm test` em `apps/desktop`: 26 testes passaram.
- `npm run build` em `apps/desktop`: TypeScript e Vite passaram.
- `cargo run -p bindforge-daemon --features kde-dbus --locked --offline -- --native-self-test`:
  interrompido por EACCES ao abrir o nó virtual de teste, antes de emitir eventos.
  A fixture foi removida, sem dispositivos `BindForge Test` remanescentes
  no sysfs. Não foram alteradas permissões.

O teste virtual isolado e a ativação física têm requisitos diferentes: o teste
precisa ler os novos nós virtuais para verificar a saída; o remapeador físico
abre o mouse cadastrado e escreve em uinput. O erro do teste isolado não equivale
a falha na sessão física já ativa.

## Procedimento para repetir o teste manual

1. Alternar entre um aplicativo associado e o BindForge. Conferir o histórico do
   daemon e os efeitos dos laterais; selecionar um perfil para edição não troca
   o perfil ativo.
2. Manter um lateral pressionado durante a troca para um perfil com ação diferente.
   A ação anterior deve ser liberada. Soltar e pressionar novamente deve aplicar
   a ação do novo perfil, sem modificadores presos.
3. Desconectar o receptor USB durante a captura e conferir a pausa. Ao reconectar,
   atualizar o inventário e refazer o vínculo caso a interface tenha mudado;
   a captura só deve retornar após ativação explícita.
4. Com a captura ativa, encerrar o daemon com Ctrl+C no seu terminal e conferir
   o comportamento original do mouse. Reiniciar o daemon deve deixá-lo pausado.

Os cenários físicos desta rodada foram confirmados pelo usuário. A consulta final
encontrou o novo daemon como usuário comum, ponte conectada e captura novamente
ativa; a observação do estado pausado logo após iniciar veio da confirmação do
usuário. Bloqueio de sessão e suspensão foram implementados na etapa posterior;
a validação física específica passou e está registrada separadamente. Esta rodada não
mede latência, não valida outros modelos de mouse e
não substitui o teste virtual que exige leitura dos nós de teste.

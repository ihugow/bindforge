# Backend experimental de mouse no Linux

O daemon `--kde` disponibiliza o backend, mas sempre inicia **pausado**. A UI oferece
**Início → Remapeamento do mouse**, seleção de um cadastro salvo e ações **Ativar**
e **Pausar remapeamento**. Abrir o app, carregar perfis e detectar janelas não ativa
captura física. Reiniciar o daemon também não retoma uma captura anterior.

## Escopo

Uma interface física com funções de mouse por vez, selecionada por ID de cadastro.
O daemon resolve o ID no inventário udev atual: o frontend não fornece caminhos.
Interfaces compostas de mouse e teclado, como a do Logitech G PRO Wireless, são
aceitas quando oferecem BTN_LEFT e os eixos relativos X/Y. Anunciar KEY_A ou
controles multimídia não invalida um mouse. Interfaces virtuais, saídas do app e
teclados sem ponteiro são recusados. O descritor aberto tem seu número de
dispositivo conferido novamente.
Não há hotplug automático: desconexão pausa e requer nova ativação. Ausência de
janela/foco também pausa; ativar exige foco identificado pela ponte KWin.
Bloqueio, inatividade da sessão e suspensão também pausam o remapeamento via
logind. O retorno exige ativação explícita; falha do monitor impede a captura.
Veja [comportamento e validação da sessão](session-lifecycle.md).

São preservados movimentos relativos, rolagem (incluindo alta resolução) e botões
sem mapeamento. Os códigos simbólicos 1/3/2 correspondem a BTN_LEFT/RIGHT/MIDDLE;
8 a 12 correspondem a BTN_SIDE/EXTRA/FORWARD/BACK/TASK. Outros controles ainda não
podem ser remapeados pelo backend. Teclas e controles multimídia da interface
compartilhada passam pelo motor sem remapeamento. ABS_VOLUME é replicado na saída
com seus limites originais; outros eixos absolutos e tipos de entrada desconhecidos
continuam recusados. Capacidades LED e REP anunciadas pelo HID++ não impedem
a captura: feedback de LEDs e configuração de repetição pelo host não são
espelhados. Repetições físicas EV_KEY com valor 2 continuam sendo encaminhadas
para teclas sem mapeamento.

Ações: tecla/atalho físico Linux, outro botão e desativação. Teclas são códigos
físicos, não texto Unicode ou mudança de layout; o layout do sistema determina o
caractere. Há aliases Ctrl, Shift, Alt, Super, Left etc. e KEY_* abaixo de BTN_0.
Atalhos vazios, desconhecidos e com aliases duplicando a mesma tecla são recusados.
Autorrepetição de atalhos não está implementada; pressionar/soltar e estado mantido
para jogos são emitidos. O editor de teclado continua pendente.

## Transporte e recuperação

Usa [evdev 0.13.2](https://docs.rs/evdev/0.13.2/evdev/) com APIs seguras: o código do
projeto mantém `unsafe_code = forbid`. Uma tarefa lê o mouse por evdev em modo não
bloqueante e envia quadros para uma saída uinput própria. A captura EVIOCGRAB só é
feita depois de validar a interface, compilar o plano e criar a saída virtual.
Exige todos os botões soltos ao ativar. Se outro remapeador já possui o dispositivo,
a captura é recusada; pause o Keymasq nesse mouse antes de ativar nosso backend.

A tarefa de entrada usa espera de 2ms quando não há eventos, independente da espera
KWin de 500ms. Quadros são emitidos ao SYN_REPORT. SYN_DROPPED, erro de leitura,
quadro acima de 4096 eventos ou falha de saída encerram o remapeamento e liberam
entrada/saída. Não há medição de latência de ponta a ponta nesta etapa.

O motor calcula a ação no pressionamento e mantém referências por tecla de saída:
dois botões compartilhando Ctrl não o liberam enquanto o outro ainda precisa dele.
Uma troca de perfil libera ações alteradas e bloqueia o botão físico já pressionado
até sua soltura. Ações iguais continuam mantidas, incluindo arrastar com o botão
esquerdo. A saída nunca é realimentada ao motor.

Planos inválidos não substituem o anterior. Falha irrecuperável de I/O encerra o
backend. Uma troca de foco para perfil incompatível pausa o remapeamento e permite
acompanhar o novo perfil sem captura. Configuração inválida durante a recarga é
reportada e não substitui o plano anterior. Alterar cadastros de dispositivos pausa
a captura para exigir nova seleção explícita.

Pausar, SIGINT/Ctrl+C e SIGTERM liberam teclas sintéticas, destroem a saída e fecham
a captura. Fechar somente a UI mantém o daemon ativo. Encerramento abrupto fecha os
descritores no kernel, mas recuperação de cada aplicativo após remoção do dispositivo
virtual ainda requer validação com hardware. O projeto não instala regras udev,
não altera ACLs/grupos e não executa o daemon como root.

## IPC e indicação de modo

`org.bindforge.Daemon.Input1` no caminho `/org/bindforge/Daemon`:

- `GetInputStatus() -> (u, b, s, s)`: versão 1, ativo, ID cadastrado e mensagem.
- `StartMouse(s) -> ()`: somente um cadastro de mouse conhecido pelo daemon.
- `PauseMouse() -> ()`: libera captura e saída.

Esses comandos não aceitam código, caminhos de dispositivo ou eventos arbitrários.
O cliente fixa o dono único do daemon. O campo `simulation` de Status1 agora também
aceita false, indicando captura nativa ativa; true significa entrada pausada neste
modo. Mudanças de modo e erros de entrada também publicam StatusChanged. A UI usa
esses eventos para renovar o estado de entrada, sem polling contínuo.

## Teste virtual isolado

```bash
cargo run -p bindforge-daemon --features kde-dbus -- --native-self-test
```

Cria somente duas fixtures uinput com nomes BindForge Test Input/Output.
Antes de emitir qualquer evento, o teste captura **ambos** os nós com exclusividade:
o motor recebe só a entrada de teste e um leitor de teste recebe só a saída. Leitores
observadores verificam que nenhum evento chegou a outros clientes. A exclusividade
segue o contrato [input_grab_device do kernel](https://docs.kernel.org/driver-api/input.html).
Não captura periféricos físicos e não troca o perfil/configuração do usuário.

Verifica botão lateral → V pressionar/soltar, troca para C enquanto o botão fica
pressionado, liberação de V e aplicação de C após soltar/repressionar. As fixtures
são destruídas durante a limpeza, com a entrada destruída antes de soltar seu grab,
e a saída protegida pelo leitor exclusivo até sua destruição.

**Limite encontrado nesta máquina:** a criação uinput foi permitida, mas a abertura
do novo `/dev/input/event29` foi negada (EACCES). O teste parou antes de emitir eventos
e a fixture foi removida. Não foram alteradas permissões para contornar isso. O
usuário posteriormente executou o teste isolado com sudo e informou PASS para
o fluxo completo de kernel. Na rodada física de 23 de outubro, a captura do
Logitech G PRO Wireless foi observada com o daemon como usuário comum; o usuário
confirmou os atalhos laterais sem teclas presas, a pausa/reativação pela UI, a
troca de ações entre Padrão e Arc Raiders (inclusive com um lateral pressionado)
e a pausa/reativação após desconectar o receptor. O encerramento com Ctrl+C e o
reinício pausado também foram confirmados. A saída virtual foi removida ao encerrar.
Veja as evidências e os limites no [registro da rodada física](hardware-validation.md).
O daemon de uso normal continua sendo executado sem sudo.

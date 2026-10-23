# Adaptação Fedora KDE Plasma 6 / Wayland

## Ponte de foco implementada (experimental)

`platform-kde` oferece `KdeWindowMonitor` com a feature `dbus`; o daemon a habilita
por `--features kde-dbus` e seleciona o monitor por `--kde`. Apenas observa foco,
sempre usando `DryRunRemapper`. Não lê `/dev/input` nem injeta eventos.

O serviço é registrado **no bus de sessão**, com:

- Nome: `org.appbuttons.Remap`
- Caminho: `/org/appbuttons/Remap`
- Interface: `org.appbuttons.Remap.Focus1`
- Método atual: `FocusChangedWithProcess(desktop_file_name: s, resource_class: s, process_id: s) -> ()`.
  O PID é decimal, transitório e não é persistido. A string evita conversões ambíguas
  de números JavaScript no D-Bus. O método legado `FocusChanged(s, s)` continua aceito,
  sem resolução por processo.

Cada chamada compara o remetente único da mensagem com o dono atual de
`org.kde.KWin`, consultado no próprio bus. Ausência de KWin ou remetente diferente
retorna `org.freedesktop.DBus.Error.AccessDenied`. Cada campo admite até 4096 bytes,
sem caracteres de controle; entradas inválidas retornam `InvalidArgs`.
Não há parâmetro para desativar autenticação. Um segundo daemon falha ao adquirir
o nome, sem entrar numa fila e assumir o serviço silenciosamente depois.

A caixa de entrada da aplicação guarda somente o evento autenticado mais recente,
coalescendo mudanças intermediárias. Sequências impedem que consultas de autenticação
que terminem atrasadas sobrescrevam uma atualização mais nova já aceita. A fila de
mensagens zbus é configurada para 64; isso não constitui um limite global de memória
ou proteção contra DoS por chamadas concorrentes. Limitação de taxa ainda é pendente.

O monitor consulta o dono KWin ao consumir atualizações e enquanto espera (intervalo
nominal de 500 ms). Mudança/perda de dono emite ausência de foco, limpando a seleção
anterior. Mensagens de um dono antigo não são aplicadas. A nova identidade é
reavaliada e uma atualização nova pode ser consumida após o fallback. As chamadas
D-Bus têm timeout de 2 segundos; desconexão/erro de saúde encerra o daemon com reset.
O polling não detecta uma perda e retomada do nome pelo mesmo remetente inteiramente
entre consultas; sinais de ciclo de vida e bloqueio de sessão ainda serão necessários.
A fronteira de confiança é a sessão do usuário, não isolamento de processos hostis
que executem com a mesma identidade e possam controlar/substituir o compositor.

`KwinFocus` prefere `desktopFileName`, depois `resourceClass`; ambos vazios indicam
sem foco. Valores são preservados exatamente. `desktopFileName` também pode ser um
caminho `.desktop` completo; normalização, Flatpak e mudança de metadados de uma
janela já ativa ainda precisam de testes/definição. Não se infere executável da
classe e não se coleta título.

## Ponte temporária gerenciada pelo daemon

```bash
cargo run -p remap-daemon --features kde-dbus -- --kde
```

O receptor adquire o nome exclusivo antes de criar arquivos ou carregar scripts.
No primeiro ciclo, `BridgeRuntime` usa `/Scripting.loadScript` e chama `run` no
`/Scripting/ScriptN` retornado. A fonte é incorporada ao daemon com `include_str!`;
não há pacote, download ou necessidade do checkout em execução. O script publica
um snapshot inicial e as próximas ativações de janela.

O nome reservado é `app-buttons-remap-runtime`. O arquivo fica em uma pasta 0700
sob XDG_RUNTIME_DIR absoluto, ou no diretório temporário, identificada pelo UID.
A pasta existente precisa ser um diretório real, do usuário atual, sem acesso a
outros usuários. O arquivo é escrito em 0600 e renomeado, substituindo uma fonte
remanescente. Links simbólicos para a pasta são rejeitados.

O monitor verifica o dono KWin e a presença da ponte enquanto espera os eventos.
Ausência/troca de compositor limpa a seleção anterior; uma nova instância recebe
uma carga automática da ponte e um novo snapshot. Falhas de carga são expostas no
diagnóstico e repetidas com pelo menos três segundos entre os inícios de tentativas
automáticas. Reativar a ponte não chama `Scripting.start` nem reconfigure global.
Como [unloadScript usa deleteLater](https://github.com/KDE/kwin/blob/master/src/scripting/scripting.cpp),
a recarga aguarda a destruição anterior com prazo limitado antes de `loadScript`.

Ctrl+C e SIGTERM são tratados pelo daemon: o loop para, reseta a simulação, publica
estado encerrado e o Drop do monitor descarrega apenas a ponte runtime e remove a
fonte temporária. SIGKILL, falha abrupta ou queda de energia não executam Drop. Na
próxima execução, o daemon descarrega uma ponte runtime remanescente pelo nome
reservado e substitui sua fonte. Não se promete limpeza imediata após SIGKILL.

## Migração da ponte persistente anterior

Antes de carregar a ponte runtime, o daemon verifica `app-buttons-remap-focus`.
Se o pacote local existir ou o plugin estiver carregado, lê sua chave com
kreadconfig6 e grava somente `Plugins/app-buttons-remap-focusEnabled=false` com
kwriteconfig6 se necessário. Confirma a gravação e descarrega esse plugin. O pacote
instalado é preservado, assim como outras opções e scripts. Uma falha impede a carga
duplicada e aparece no diagnóstico, com possibilidade de nova tentativa.

O funcionamento normal não depende de kpackagetool6 nem grava kwinrc. As ferramentas
KConfig são usadas somente na migração, sem shell e sem sudo. Reabilitar o script
legado enquanto o daemon está aberto faz a integração desativá-lo novamente para
preservar uma única ponte. Para instalações globais carregadas, a preferência do
usuário também é gravada como false; não é necessário remover arquivos do sistema.

## Diagnóstico no aplicativo nativo

Em **Início → Ver integração com KDE**, o app consulta o daemon por
`org.appbuttons.Remap.Integration1` no mesmo caminho do serviço. A resposta de
`GetIntegration` é `(u, b, b, b, b, s, t)`: versão 1, KWin disponível, ponte carregada,
foco recebido desde a última carga, migração registrada, mensagem e número de
reconexão concluída. Nenhum ID/título de janela ou caminho temporário é exposto.

`RequestReconnect() -> t` registra uma solicitação tipada; chamadas simultâneas
pendentes são agrupadas. O loop do daemon processa a recarga. A UI acompanha a
conclusão com um cliente preso ao dono do daemon, com prazo de 12s, e oferece
**Verificar novamente** se o prazo vencer. Não recebe comandos, código ou caminhos
do frontend. O método Tauri `configure_kwin_integration` agora solicita apenas a
reconexão; não instala ou habilita scripts.

O diagnóstico diferencia daemon indisponível, KWin ausente, ponte conectada e foco
recebido. O foco vem de eventos autenticados e é zerado em perda/recarga da ponte;
não é uma promessa de entrega futura ou um teste de remapeamento de entrada. Um
daemon antigo é identificado com orientação de atualização/reinício. Fechar apenas
a UI mantém a ponte ativa enquanto o daemon estiver em execução.

## Smoke test da sessão real

```bash
cargo build -p remap-daemon --features kde-dbus --locked
python3 tools/test-kde-session.py
```

O teste opt-in recusa substituir um daemon ativo e recusa migrar uma ponte legada
habilitada. Inicia um daemon filho, confirma seu PID, verifica rejeição de remetente
não autorizado e aguarda a carga/snapshot automáticos. Envia SIGTERM ao filho e
confirma que a ponte foi descarregada. Não ativa janelas nem imprime metadados de
aplicativos. `--check-status` permanece como opção de compatibilidade; a consulta
de estado é sempre verificada. Nenhum teste desta etapa reiniciou o KWin real.

## Verificação e próximos passos

```bash
cargo test --workspace --all-features --locked
node --test integrations/kwin/tests/focus.test.cjs
```

O teste Rust cria seu próprio `dbus-daemon` e clientes simulados, sem registrar nomes
na sessão KDE real. Cobre remetente não autorizado, campos inválidos, fallback,
coalescência, perda/reaquisição do compositor, bus encerrado e nome já ocupado.
O teste JS verifica snapshot inicial e argumentos de mudanças/ausência de foco.

A consulta de estado usa `Status1` (ver [status-ipc.md](status-ipc.md)). A ponte
reconecta automaticamente e o backend evdev/uinput está implementado para um mouse.
Bloqueio/desbloqueio e suspensão foram validados em sessão real; consulte
[session-lifecycle.md](session-lifecycle.md) e [hardware-validation.md](hardware-validation.md).
Continuam pendentes controle de taxa específico do receptor, medição de latência
e cobertura ampliada de apps Wayland/XWayland, Flatpak, diálogos e hardware.

## Entrada real e execução na sessão

A implementação usa evdev/uinput, com supressão do evento original, liberação de
ações mantidas e ativação explícita de um mouse cadastrado. A UI e o daemon rodam
como usuário comum; não há alteração automática de ACLs, grupos ou regras udev.
Firmware, DPI/RGB e macros executáveis continuam fora do escopo atual.
Veja [native-input.md](native-input.md) para capacidades e limites, e
[desktop-service.md](desktop-service.md) para execução sem terminal e login opcional.

## Identidade individual para Gamescope por jogo

O script 0.1.1 transmite o PID da janela. Após autenticar o remetente KWin, o daemon
verifica janelas identificadas como `gamescope` no adaptador Linux: mesmo proprietário
do daemon, executável com nome `gamescope` e leitura limitada de `/proc/<pid>/environ`.
Somente a chave `SteamAppId` é interpretada; outros valores são descartados e nunca
registrados. Se a variável estiver ausente, o adaptador também reconhece o marcador
explícito `reaper SteamLaunch AppId=<id> --` no comando filho após o separador `--`
do Gamescope. Outras sequências de argumentos são ignoradas, e IDs válidos
divergentes entre as duas fontes são rejeitados. O início do processo é conferido antes/depois da leitura para rejeitar
reutilização de PID. Um ID positivo u32 gera a identidade normalizada `steam:<id>`,
usada tanto pela captura quanto pelo core de seleção, sem mudar o formato dos perfis.
A consulta roda fora do executor assíncrono D-Bus.

Sem PID, sem acesso ao processo, em caso de identificador inválido ou de namespace
incompatível, a identidade continua `gamescope`. A UI rejeita esse resultado como
captura individual e avisa, mantendo disponível a associação genérica manual. Não
há busca por processos globais, descendentes ou títulos, nem inferência pelo nome
de executáveis de jogos. Um Gamescope compartilhado que hospeda vários jogos não
pode ser individualizado pela identidade do processo externo nesta implementação.

É preciso atualizar o pacote de script instalado e reativá-lo, além de reiniciar
o daemon/interface. O script novo exige o método novo no daemon; atualizar apenas
um dos processos não habilita a resolução por jogo. A combinação Steam/Flatpak e o
jogo real precisam de validação no ambiente do usuário.


### Correção validada no processo ARC Raiders real

Na sessão KDE observada, o Gamescope é uma janela com PID válido, mas ambos os
campos `desktopFileName` e `resourceClass` chegam vazios. O adaptador agora tenta
resolver o PID também nesse caso, antes de concluir que não existe janela ativa.
Um processo de outro aplicativo com rótulos vazios não é convertido em Gamescope.

A instalação local tem `cap_sys_nice=ep` no Gamescope. Nesse estado, a leitura de
`/proc/PID/exe` e `environ` pode ser negada mesmo para o mesmo usuário, enquanto
`cmdline`, `comm` e `stat` permanecem disponíveis. Quando o link do executável é
negado por permissão, a identificação exige o mesmo UID, argv[0] com basename
`gamescope`, comm `gamescope`/`gamescope-wl` e início de processo consistente. Ela
pode então usar o marcador explícito Steam no comando, sem alterar permissões.

O comando observado contém `steam-launch-wrapper -- reaper SteamLaunch AppId=...`.
Esse invólucro conhecido é aceito além do formato direto com reaper; outros
invólucros/argumentos arbitrários não são varridos. O diagnóstico usando a mesma
biblioteca retornou `steam:1808500` para o processo real do ARC Raiders aberto.
O daemon em execução precisa ser reiniciado para consumir a correção. O script
KWin 0.1.1 já instalado não precisa de nova alteração nesta correção.

Diagnóstico opcional, imprimindo somente a identidade resolvida:

```bash
cargo run -p platform-linux --example identify-gamescope -- PID_DO_GAMESCOPE
```

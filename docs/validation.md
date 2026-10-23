# Verificação — 23 de outubro de 2026

Ambiente: Fedora 44 x86_64, Rust/Cargo 1.99, Node 22.23.1, npm 10.9.8.
Core com MSRV declarado 1.82; daemon/adaptador agora declaram Rust 1.87 por causa
de zbus 5.19 e suas dependências. Nenhum dos toolchains mínimos foi executado.
Windows/macOS não foram compilados/testados.

## Etapa 2: ponte de foco D-Bus

| Verificação | Resultado |
| --- | --- |
| `cargo test --workspace --all-features --offline` | 23 testes passaram: 9 core, 3 adaptador, 1 integração foco, 4 protocolo/one-shot daemon, 3 CLI, 1 integração estado, 2 IPC |
| `cargo test --workspace --locked --offline` | 19 testes passaram com features padrão; integração D-Bus não é habilitada |
| `cargo check --workspace --all-targets --all-features --locked --offline` | Passou |
| `node --test integrations/kwin/tests/focus.test.cjs` | 1 teste passou: foco inicial, troca e ausência de foco, sem títulos |
| `npm run build` em apps/desktop | TypeScript e Vite passaram após atualização dos textos de status |
| `remap-daemon --help` | Opção `--kde` documentada; exige a feature opcional |

Os testes Rust usaram um cache Cargo temporário em `/tmp/remap-cargo-home`.
O teste de integração cria seu próprio `dbus-daemon` e clientes simulados: não usa
`DBUS_SESSION_BUS_ADDRESS` da sessão KDE. A criação do socket foi bloqueada pelo
sandbox inicial e o teste completo foi executado com autorização fora dele.
O diretório runtime do bus de teste é temporário e removido no encerramento.

O teste de integração confirmou: ausência de KWin/remetente incorreto retornam
AccessDenied; payload grande/caracteres de controle retornam InvalidArgs;
coalescência preserva o último evento aceito; perda e nova aquisição do nome KWin
limpam a seleção; desconexão do bus retorna erro; segundo receptor não assume/fica
na fila do nome de serviço. O teste detectou que o builder de zbus não rejeitava
sozinho a segunda instância; a implementação agora verifica explicitamente
`RequestName` com `DoNotQueue` e exige uma resposta de proprietário.

## Verificações anteriores e limites atuais

A demo padrão e via stdin passaram com a sequência default → firefox → konsole →
default. A prévia React foi inspecionada no navegador na etapa inicial; a consulta
nativa permanece desabilitada no navegador. Não houve nova inspeção visual para
a alteração de texto desta etapa.

`npm run tauri info` reconheceu a configuração, mas faltam WebKitGTK 4.1 e librsvg
de desenvolvimento. O shell nativo não foi compilado: a tentativa offline inicial
não encontrou `tauri` no cache. O build web não comprova compilação ou IPC no shell.
`cargo fmt` e `cargo clippy` continuam indisponíveis neste ambiente.

## Etapa 3: snapshot inicial no Plasma real

`python3 tools/test-kde-session.py` passou na sessão do usuário, Fedora 44/Wayland,
KWin 6.7.5. O teste usou o script de produção sem modificá-lo, carregado via D-Bus
com nome exclusivo e temporário. Confirmou:

- Nome do receptor pertence ao PID do daemon filho iniciado pelo teste.
- Cliente D-Bus comum recebe AccessDenied ao tentar enviar foco.
- Script executado pelo KWin envia um snapshot inicial aceito pelo receptor.
- Daemon consome um evento, executa reset simulado e sai com sucesso.
- Dono KWin permaneceu o mesmo durante o teste.
- Script temporário foi descarregado; nome do daemon não tinha mais dono ao final.

A saída de seleção foi `simulation profile=default`. Não foi necessário registrar
qual aplicativo estava ativo. O teste não ativa janelas e não comprova ainda a troca
de perfis entre janelas reais. Os dois testes novos cobrem consumo de exatamente um
evento e propagação de erro no modo `--once`. Nenhum remapeamento físico ocorreu.

Nenhum serviço persistente, pacote KWin, regra udev ou pacote de sistema foi
instalado. Houve somente carregamento temporário do script para o teste real. Dependências Rust foram baixadas do crates.io. O remapeamento físico,
bloqueio/desbloqueio da sessão, limite de taxa, snapshot automático após reconexão
e encerramento por sinal com reset explícito não estão implementados/testados.
A consulta IPC de estado está implementada; a validação no shell Tauri ainda é pendente. Não habilitar captura/injeção de entrada
até implementar e validar esses fluxos.


## Etapa 4: consulta de estado UI–daemon

- Crate `remap-ipc`: store versionado e interface `Status1.GetStatus`, sem métodos
  de escrita; o daemon publica após consumir eventos/aplicar o perfil simulado.
- A integração nova executa um daemon real em bus privado. O cliente Rust usado
  pelo Tauri leu fallback, Firefox, app desconhecido e ausência de foco; consultas
  não alteraram o estado e `SetStatus` retornou UnknownMethod. Após matar e reiniciar
  o processo, o cliente falhou durante a ausência e leu a revisão nova ao reconectar.
- `npm test`: 2 testes de polling passaram (falha remove perfil antigo, reconexão
  aceita revisão reiniciada, uma consulta por vez e descarte após desmontagem).
- `npm run build`: TypeScript e Vite passaram. O layout do painel foi inspecionado
  no navegador; nele a consulta nativa permanece desabilitada, sem dados simulados
  apresentados como se fossem do daemon real.
- `python3 tools/test-kde-session.py --check-status`: passou na sessão real. A revisão
  e o estado de foco mudaram após receber o snapshot KWin. O script temporário foi
  descarregado e o daemon filho encerrado.
- O comando Tauri está implementado, mas a compilação/execução nativa e o fluxo
  completo React → comando → D-Bus permanecem sem validação neste ambiente, que
  não tem os pacotes de desenvolvimento GTK/WebKit. Nenhum pacote do sistema foi instalado.


## Correção: mudanças desapareciam ao voltar para a janela do app

O usuário confirmou que a interface nativa estava conectada e forneceu logs reais
alternando `konsole` e `default`. A consulta direta a `Status1.GetStatus` também
retornou estado válido. O painel mostrava somente o perfil atual: retornar para a
janela do app escolhia novamente `default`, ocultando a passagem pelo Konsole.

Foi adicionado um histórico no daemon, limitado a 12 seleções diferentes e somente
em memória, com snapshot atômico em `GetStatusWithHistory`. A UI mostra as últimas
trocas mesmo quando duas consultas consecutivas veem `default`. Para um daemon
antigo ainda rodando, mantém a consulta atual e pede reinício para habilitar histórico.

Verificação desta correção:

- `cargo test --workspace --all-features --locked --offline`: 26 testes passaram.
- `npm test`: 3 testes passaram, incluindo a passagem pelo Konsole entre consultas.
- `npm run build`: TypeScript/Vite passaram.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --offline`: passou.
  Os pré-requisitos GTK/WebKit agora estão presentes no ambiente.
- Teste de integração com daemon real em bus privado confirma histórico, ausência
  de duplicação, reconexão e fallback para um serviço legado sem o método novo.
- O daemon do usuário não foi interrompido/substituído pelo agente. Ele deve ser
  reiniciado para usar a implementação nova; a conferência visual final do histórico
  permanece pendente desse reinício. Não houve captura/injeção de entrada.


## Correção: painel conectado congelava após a primeira consulta

A imagem enviada pelo usuário mostrou o Konsole em foco, painel conectado parado
na revisão 3 e histórico somente com `default`. A consulta direta ao daemon retornou
revisão 37 e diversas entradas `konsole/default`, demonstrando que o painel estava
exibindo um snapshot antigo. A interpretação anterior como simples retorno de foco
não explicava esse congelamento.

Causa reproduzida em navegador real: o agendador armazenava `setTimeout`/`clearTimeout`
como propriedades e as chamava com `this = scheduler`. O temporizador nativo rejeitou
a chamada com `TypeError: Illegal invocation`. A primeira consulta concluía, mas
nenhuma próxima consulta era agendada. Os testes anteriores injetavam um agendador
fake e não cobriam o contexto das funções nativas.

A correção usa wrappers que chamam os temporizadores em `globalThis`. Também move
o histórico para fora do parágrafo de status, corrigindo um aninhamento HTML inválido.

- `npm test`: 4 testes passaram, incluindo preservação do receptor dos temporizadores.
- `npm run build`: TypeScript e Vite passaram.
- Fixture de navegador `tests/polling-browser.html`: antes `FAIL: TypeError: Illegal
  invocation`; depois `PASS: 4 consecutive reads and clean cancellation`.
- Nenhum código Rust/daemon foi alterado nesta correção; testes de remapeamento
  físico continuam fora de escopo. O daemon do usuário foi mantido em execução.
- Após a atualização automática, um monitor D-Bus restrito a chamadas de
  `Status1.GetStatusWithHistory` observou 5 consultas do aplicativo aberto em
  5 segundos. A fixture usa snapshots sintéticos e não gera chamadas D-Bus.

## Atualização por eventos

O polling de estado da UI foi substituído por snapshot inicial + sinal D-Bus
StatusChanged + Channel Tauri. O daemon emite o snapshot após aplicar a seleção
simulada. A UI recebe eventos sem timer de consulta; consulta individual permanece
apenas para diagnóstico. Um daemon antigo pede reinício, sem polling de fallback.

Validação:

- `cargo test --workspace --all-features --locked --offline`: 27 testes passaram.
- `npm test`: 5 testes passaram; substituem os testes do poller removido.
- `npm run build`: TypeScript/Vite passaram.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --offline`: passou.
- A integração com daemon real em bus privado verifica entrega por eventos, perda
  e retorno de dono, nova revisão, rejeição de sinais de outro processo e cancelamento.
- Fixture events-browser.html em navegador real: PASS para três atualizações
  imediatas, cancelamento limpo e descarte de evento tardio.
- Processos do usuário foram preservados. É necessário reiniciar daemon/interface
  para validar esta versão na sessão real. Nenhuma medição da latência total de
  renderização foi feita; a espera periódica de um segundo foi removida por construção.

## Inventário de mouses e teclados

- Crate platform-linux com libudev, enumeração somente de metadados e trait neutro
  DeviceEnumerator no core. Interface Devices1 no daemon e painel na UI nativa.
- Enumeração real via `--list-devices` encontrou 8 interfaces neste Fedora, incluindo
  USB e virtuais do keymasq. Nenhum canal de entrada foi aberto pelo código do adaptador.
- `cargo test --workspace --all-features --locked --offline`: 33 testes passaram.
- `npm test`: 7 testes passaram; `npm run build`: TypeScript/Vite passaram.
- `cargo check` do shell Tauri passou. A fixture visual de dispositivos foi
  inspecionada no navegador, com dados sintéticos identificados como tal.
- A integração em bus privado consulta a enumeração real pelo daemon e preserva o
  estado do perfil. Também verifica erro orientando reinício para serviço legado.
- Hotplug físico e a nova lista no shell real ainda não foram testados. A UI oferece
  atualização manual. IDs são locais à conexão e não usados para persistir perfis.
- Processos do usuário, grupos, regras udev e pacotes de sistema foram preservados.

## Criação e persistência de perfis

- `cargo test --workspace --all-features --locked --offline`: 37 testes passaram.
- `npm test`: 10 testes passaram.
- `npm run build`: TypeScript e Vite passaram.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --offline`: passou.
- A integração D-Bus usa arquivo isolado e verifica recarga sem evento de foco,
  preservação após corrupção e retorno a uma configuração válida.
- Fixture visual em navegador: criação com dois aplicativos, cópia de ações do
  Firefox, aviso de conflito, prioridade 20 e edição posterior do nome conferidos.
  O perfil manteve a ação copiada e o armazenamento em memória passou da revisão 1 à 2.
- O formulário no shell Tauri com gravação real ainda precisa de validação manual;
  os comandos nativos foram compilados e o armazenamento real foi testado em Rust.
- Rustfmt/Clippy continuam ausentes neste ambiente. Nenhum perfil do usuário foi
  gravado durante a validação automatizada/visual.

## Detecção de aplicativo pela próxima janela

- Workspace Rust com todas as features: 39 testes passaram.
- Frontend: 14 testes passaram; TypeScript/Vite compilaram.
- Shell Tauri: `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --offline` passou.
- Os testes em bus D-Bus privado cobrem o protocolo no monitor e no daemon real:
  eventos futuros, aplicativo inicial ignorado, falta de foco ignorada, primeiro
  resultado preservado, remetente falso rejeitado, cancelamento, rearmamento,
  perda de KWin/daemon e aviso para daemon antigo.
- Testes de estado do frontend cobrem resposta anterior ao retorno do comando,
  limpeza com início pendente, resposta após cancelamento, timeout e IDs inválidos.
- Fixture no navegador: painel pronto, cancelamento, foco simulado após cancelar
  sem preenchimento, rearmamento, ID preenchido, timeout, nova captura sem ID
  duplicado e salvamento do perfil em memória conferidos.
- A captura de janelas na sessão KDE real não foi exercitada nesta etapa. Os
  processos abertos pelo usuário foram preservados; reinicie daemon e interface
  para validar esta versão. Nenhum perfil real foi gravado nos testes visuais.

## Identificação de jogos Steam dentro de Gamescope

- `cargo test --workspace --all-features --locked --offline`: 43 testes passaram.
- Frontend: 15 testes; build TypeScript/Vite passou.
- Shell Tauri: verificação de compilação passou; o shell não precisa acessar /proc.
- Ponte KWin: teste JavaScript passou para foco inicial, PID decimal, ausência de
  foco e PID indisponível, sem transmitir títulos.
- Testes Linux validam apenas SteamAppId positivo, marcador explícito de lançamento
  Steam após o separador Gamescope, conflitos, ausência de metadados, executável
  inesperado e limites de leitura. Não há busca em processos não associados à janela.
- A integração D-Bus usa dois processos reais de teste com executável chamado
  gamescope e SteamAppId distintos. As leituras reais em /proc geram steam:1808500
  e steam:570; captura e monitor entregam identidades individuais. Um PID Steam
  não relabela uma janela Firefox. Remetente não autenticado e PID inválido são
  rejeitados. O core seleciona perfis diferentes para essas duas identidades.
- A UI não acrescenta o ID genérico gamescope ao formulário por captura. Continua
  possível incluí-lo manualmente, com aviso de associação genérica.
- A inicialização real do ARC Raiders informada pelo usuário ainda precisa ser
  testada. Nenhum Gamescope real estava aberto durante a inspeção. O pacote KWin
  instalado e os processos do usuário não foram alterados; atualizar/reativar o
  script e reiniciar daemon/interface é necessário para habilitar o novo caminho.


## Nome do perfil selecionado e do histórico

- Corrigida a exibição que usava o ID interno como rótulo do perfil.
- O painel de perfis comunica a configuração após carga, recarga e salvamento;
  o painel de estado e o histórico resolvem o nome usando a mesma configuração.
- `npm run build`: TypeScript/Vite passaram; os 15 testes existentes do frontend passaram.
- Fixture visual: o ID `profile-b4985e28-8d9d-41ab-a04a-868fb56aa0f9` aparece como
  “Marvel Rivals” no perfil selecionado e na última troca. Renomear para “Marvel
  Rivals · personalizado” atualiza imediatamente os dois rótulos após salvar.
- Nenhum perfil real foi editado. Não houve mudanças no daemon, protocolo ou ponte KWin.

## Timeout da captura no ARC Raiders com Gamescope — diagnóstico real

- Observados os eventos reais da ponte: a janela do jogo transmitia os dois IDs
  vazios, mas com PID válido. O daemon interpretava isso como NoActiveWindow e
  a captura aguardava até expirar.
- Confirmado Gamescope local com cap_sys_nice=ep: exe/environ em /proc negados por
  permissão. cmdline estava acessível e incluía o invólucro steam-launch-wrapper
  antes de reaper SteamLaunch AppId=1808500.
- Corrigidos os três casos: rótulos vazios com PID verificável, leitura alternativa
  com validação de argv0/comm quando exe é negado e wrapper explícito da Steam.
- O exemplo de diagnóstico usando a biblioteca corrigida retornou steam:1808500
  para o processo REAL do ARC Raiders aberto, sem revelar os demais argumentos ou
  o ambiente e sem alterar capacidades/permissões.
- `cargo test --workspace --all-features --locked --offline`: 44 testes passaram.
  Regressão D-Bus cobre ambos os IDs vazios com PID de Gamescope, entrega do ID à
  captura, processo de outro aplicativo ignorado e Gamescope genérico reportado
  imediatamente quando o jogo não pode ser individualizado.
- O daemon em execução foi preservado; a captura completa no aplicativo precisa
  ser repetida após reiniciar o daemon. A interface e o script KWin instalado já
  são compatíveis e não exigem alteração para esta correção.


## Organização da interface: abas e histórico recolhível

- Abas Perfis/Dispositivos com tablist/tabpanel, estado selecionado, controles
  associados e navegação por setas, Home e End.
- O histórico começa oculto e um switch Exibir histórico mostra/oculta o conteúdo;
  a assinatura do daemon continua ativa, preservando as trocas enquanto oculto.
- Perfis em edição permanecem montados ao trocar de aba. Capturas de janela abertas
  são fechadas ao ocultar a aba Perfis. Dispositivos são consultados ao abrir sua aba.
- Build TypeScript/Vite e os 15 testes existentes do frontend passaram.
- Fixture visual com componentes reais e dados sintéticos: histórico inicialmente
  oculto, exibição e ocultação pelo switch, mouse/teclado na aba separada e retorno
  para Perfis pela seta esquerda, com nome não salvo preservado, conferidos.
- Nenhuma alteração no daemon, nas associações salvas ou no script KWin.


## Exclusão de perfis

- Botão Excluir nos perfis específicos, com confirmação inline e Cancelar; o
  fallback não oferece exclusão. A remoção também é protegida na transformação
  de configuração, e a validação Rust já rejeita um conjunto sem fallback.
- Usa o mesmo salvamento atômico com revisão esperada. Erro de gravação mantém a
  lista e a confirmação; exclusão bem-sucedida atualiza nomes e limpa a prévia.
- TypeScript/Vite passaram; 16 testes do frontend passaram, incluindo proteção do
  fallback, perfil ausente e preservação das demais configurações ao excluir.
- Na fixture em memória: cancelar preservou Firefox, confirmar removeu somente
  Firefox, incrementou a revisão e recarregar manteve a remoção. Padrão e Marvel
  Rivals permaneceram disponíveis. Nenhum perfil real do usuário foi excluído.


## Formulários e confirmação em modais

- Criar/editar, captura e exclusão migrados dos blocos inline para dialog nativo,
  com títulos acessíveis, portal, fundo inerte, backdrop e scroll interno.
- O componente controla Escape, foco inicial/restauração e trava de fechamento
  enquanto salva. Modais empilhados mantêm a trava de rolagem até o último fechar.
- TypeScript/Vite e os 16 testes existentes do frontend passaram.
- Fixture visual: formulário modal com foco no nome; captura em segundo modal,
  resultado sintético firefox, Escape fechando somente a captura e foco voltando
  a Detectar pela janela; nome/ID preservados e salvamento conferidos.
- Exclusão abriu em modal com foco em Cancelar; Escape preservou o perfil e
  devolveu foco a Excluir. Edição abriu preenchida e o botão Fechar devolveu foco
  ao respectivo Editar. O fundo ficou ausente da árvore acessível enquanto modal.
- Nenhum perfil real foi alterado. Não houve mudança de protocolo nem de daemon.


## Início visual e edição de botões do mouse

- Abas Início/Perfis/Dispositivos; Início abre por padrão. Dropdown de edição
  independente da seleção automática do daemon, ilustrações SVG genéricas próprias
  de mouse e teclado, editor modal de cinco controles e modal de ação.
- Editor suporta tecla/atalho, botão de mouse, desativar e remover personalização.
  Rascunho só é persistido em Salvar alterações, com revisão esperada; fechamento
  com alterações pede confirmação. Ações de teclado preexistentes são preservadas.
- Escopo geral e específico com prioridade específica consistente no core e na UI.
  Home e Perfis sincronizam configuração após salvar/renomear.
- `npm run build`: TypeScript/Vite passaram. `npm test`: 19 testes passaram,
  incluindo escopos, preservação de outros controles, restauração à geral, teclas
  inválidas e rejeição de revisão antiga sem sobrescrever metadados.
- `cargo test --workspace --all-features --locked --offline`: 45 testes passaram.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --offline`: passou.
- Fixture em StrictMode: selecionado Marvel Rivals mantendo Padrão ativo, atribuída
  tecla V ao lateral 1 geral, interrupção por Escape protegida, ação específica
  Desativado no mesmo controle de um mouse, salvamento e contagem de 2 ações na
  aba Perfis. Renomear em Perfis preservou as ações e atualizou o dropdown Início.
- Nenhum perfil real foi alterado nos testes visuais; validação no shell nativo
  permanece manual. Não há captura/injeção nem backend de entrada física.


## Cadastro manual de dispositivos e integração com Início

- `cargo test --workspace --all-features --locked --offline`: 46 testes passaram.
  Novo teste cobre configuração legada sem dispositivos, persistência, duplicatas,
  contagem inválida e layouts aceitos/rejeitados.
- `npm test --prefix apps/desktop`: 22 testes passaram. Cadastro, compatibilidade,
  duplicatas, propriedades, edição offline, transferência de ações por tipo,
  conflitos no destino e geração dos controles adicionais foram cobertos.
- Build TypeScript/Vite e check do shell Tauri passaram.
- Fixture `tests/home-browser.html`, em StrictMode e com API somente em memória:
  cadastro de Logitech G PRO Wireless com sete botões, cadastro Aula F99 com ANSI,
  cartões no Início e abertura da interface correta com sete controles. Ação Disabled
  em botão extra salva sem perder o cadastro. Nenhum perfil real foi modificado.
- A interface pode mudar de identificador após reconectar/reiniciar; vínculo manual
  é recuperável pela edição do cadastro. Não houve remapeamento físico nesta etapa.


## Assistente de integração KDE na UI

- Shell Tauri: três testes passaram. kpackagetool6 instala o pacote incorporado em
  uma raiz temporária, repete sem mudanças e repara conteúdo desatualizado; leitura/
  gravação KConfig preserva outras chaves. Um bus D-Bus privado com mock de KWin
  valida consulta, unload/load somente do nosso plugin e run do ID retornado.
- `npm test --prefix apps/desktop`: 24 testes passaram, incluindo combinação de
  estados instalada/habilitada/carregada/daemon/foco e ações instalação/atualização/
  reativação. TypeScript/Vite e compilação do shell passaram.
- `tests/integration-browser.html` usa API em memória: primeira instalação, sucesso,
  botão Reativar ponte e indisponibilidade fora do KWin foram verificados no browser.
- Introspecção somente de leitura confirmou métodos e caminhos na sessão KWin real.
  A instalação/reativação do script do usuário não foi executada durante os testes;
  configurações reais e estado da ponte foram preservados.


## Ponte temporária automática do daemon

- `cargo test --workspace --all-features --locked --offline`: 51 testes passaram.
- Novo ciclo completo em bus privado: carga e snapshot autenticado automáticos,
  remoção de ponte legada/remanescente, pedido de reconexão via IPC, recuperação de
  descarga manual e troca do dono KWin, falha de carga com diagnóstico, nova tentativa
  e limpeza no Drop. Um script não relacionado permanece intocado.
- Processo real do daemon em bus privado: SIGTERM e SIGINT produzem saída com
  sucesso, unload da ponte e remoção da fonte temporária. Arquivos e preferências
  dos testes ficam em diretórios isolados.
- Migração idempotente de kwinrc preserva outras chaves; fonte remanescente é
  substituída e diretório simbólico é recusado. Pedidos pendentes coalescem e versões
  incompatíveis do diagnóstico são recusadas.
- Frontend: 24 testes passaram. TypeScript/Vite e cargo check do shell passaram.
  A fixture de diagnóstico usa dados em memória e oferece Reconectar ao KDE.
- `tools/test-kde-session.py` foi adaptado à carga automática e passou em py_compile.
  O smoke com KWin real não foi repetido; daemon, script instalado e configurações
  reais do usuário foram preservados. A migração real ocorrerá ao iniciar o novo daemon.


## Backend experimental nativo de um mouse

- `cargo test --workspace --all-features --locked --offline`: 56 testes passaram.
  O motor virtual em memória cobre suprimir o original, pressionar/soltar, preservar
  movimento, referências de modificadores compartilhados, ausência de realimentação,
  troca com botão mantido, preservar arrasto não alterado e rejeitar planos inválidos.
- Sessão inicia pausada, não seleciona dispositivos por caminho arbitrário, preserva
  diagnóstico ao trocar perfis enquanto pausada e exige foco válido antes de ativar.
  IPC em bus privado confirma estado pausado e rejeição de seleção por caminho.
- Cliente de status agora aceita simulation=false, para representar captura nativa.
- Frontend: 24 testes passaram; build TypeScript/Vite e cargo check Tauri passaram.
  Fixture `tests/native-input-browser.html` valida ativar/pausar e bloquear troca de
  seleção enquanto ativa, usando exclusivamente API em memória.
- Teste opt-in real: `cargo run -p remap-daemon --features kde-dbus --offline -- --native-self-test`.
  Criou uma fixture uinput e aguardou udev, mas RawDevice::open no novo event29
  retornou EACCES. Parou antes do primeiro evento, a fixture foi destruída e a
  verificação sysfs confirmou ausência de App Buttons Remap Test remanescentes.
  Não foram concedidas permissões, modificadas ACLs/regras/grupos ou capturados
  periféricos físicos. O teste completo no kernel e a validação física estão pendentes.
- O harness só emite depois de capturar entrada e saída virtuais exclusivamente,
  verifica observadores sem eventos e destrói fixtures na limpeza. Esse caminho
  ainda não passou no kernel deste ambiente por causa do EACCES acima.


## Compatibilidade com mouse composto Logitech

A interface física Logitech G Pro anuncia ID_INPUT_MOUSE e ID_INPUT_KEYBOARD,
com KEY_A, ABS_VOLUME, LED e REP. A validação passa a aceitar mouse composto
com BTN_LEFT e REL_X/Y e replica ABS_VOLUME; preserva teclas/mídia sem mapeamento.
Dispositivos virtuais, interfaces sem ponteiro, outros eixos absolutos e tipos
de entrada desconhecidos continuam recusados. Testes determinísticos cobrem
essas capacidades e a passagem de teclas, repetição e volume durante remapeamento.
Não foi ativada captura física durante a implementação. O usuário informou PASS
no teste virtual isolado anterior executado com sudo.

Validação da correção: 59 testes Rust do workspace e 24 testes do frontend
aprovados; build do daemon com kde-dbus e build TypeScript/Vite concluídos.
`git diff --check` sem erros.


## Indicador de bateria dos periféricos

Implementada leitura somente de metadados power_supply com associação pela árvore
física, IPC aditivo e campo opcional no inventário Tauri. Cards em Início e
Dispositivos mostram porcentagem, carga completa/carregando, aviso de bateria baixa
até 20% e indisponibilidade quando não há dados. Atualização a cada 60 segundos
nas abas visíveis, retorno à janela e atualização manual; nenhuma leitura é salva.

Verificações: 62 testes Rust do workspace e 26 testes do frontend aprovados;
build do daemon, TypeScript/Vite e cargo check do Tauri concluídos. O teste IPC
em barramento privado confirmou a leitura real de uma interface com 28% sem
capturar entrada ou alterar o perfil. Conferência visual em Chrome headless com
fixture sintética de Logitech com 28% e Aula sem bateria disponível aprovada.

## Rodada de validação com mouse físico

O daemon existente foi encontrado como usuário comum, com captura física do
Logitech G PRO Wireless ativa e ponte KWin conectada. O usuário confirmou os
atalhos Win+H/Win nos laterais sem teclas presas, a pausa/reativação pela UI e a
troca de perfil/ações entre Padrão e Arc Raiders. Os sinais IPC também confirmaram
a pausa/reativação e a seleção automática do perfil do jogo com captura ativa.
O usuário confirmou também a troca com um lateral pressionado, sem ações presas,
e a pausa/reativação após desconectar e reconectar o receptor USB.
O encerramento com Ctrl+C devolveu o comportamento original do mouse; o processo,
o serviço D-Bus e a saída virtual anteriores desapareceram. O usuário confirmou
que o daemon reiniciou pausado e a UI reconectou. A consulta posterior encontrou
novo processo como usuário comum, ponte conectada e captura novamente ativa.
Os 62 testes Rust, 26 testes do frontend e o build TypeScript/Vite passaram
novamente. O self-test virtual como usuário comum continua interrompido por
EACCES antes de emitir eventos, com limpeza da fixture confirmada.

Os cenários físicos propostos naquela rodada passaram. As evidências e os limites estão em
[hardware-validation.md](hardware-validation.md).

## Pausa por bloqueio de sessão e suspensão

Implementado monitor de logind no barramento de sistema, com restrições de
ativação serializadas com StartMouse e inibidor de atraso de suspensão liberado
depois de parar o remapeador. Retorno e reconexão exigem ativação explícita.

Verificações: 64 testes Rust do workspace com todas as features, 26 testes do
frontend e build TypeScript/Vite passaram; daemon compilado com kde-dbus e
`git diff --check` sem erros. Os dois testes novos usam um logind simulado em
barramento privado, cobrindo transições de sessão, autenticação, descritores de
inibição, falhas e recuperação. Os testes de daemon usam também um barramento
de sistema privado, sem adquirir inibidores na sessão real durante a suíte.

A versão atualizada foi iniciada na sessão real como usuário comum: a ponte KWin
conectou, a sessão foi verificada e ListInhibitors confirmou o registro sleep/delay
do BindForge. A captura iniciou pausada. O usuário confirmou bloqueio/desbloqueio
com um lateral pressionado e suspensão/retorno, sem teclas presas e com retomada
somente após ativação explícita. O observador de estado confirmou as duas pausas,
a permanência em pausa após retornar e as reativações posteriores. A consulta
final confirmou a renovação do inibidor de atraso e a ponte KWin conectada.
Evidências e horários em [session-lifecycle.md](session-lifecycle.md).

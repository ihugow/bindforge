# BindForge

Base de um gerenciador de perfis para mouse/teclado em Rust. Primeiro alvo:
**Fedora KDE Plasma 6, sessão Wayland**. Windows e macOS são extensões futuras.
Implementação independente, sem código, assets ou protocolos proprietários do G Hub.

**Estado: seleção automática de perfis e backend experimental de mouse e teclado no Linux.**
O daemon inicia com entrada pausada; mouse e teclado cadastrados são ativados
separadamente pela UI. Perfis e cadastros são persistidos em JSON, com detecção de janelas pelo KWin.
O teclado Aula F99 teve o remapeamento básico confirmado em uso físico; a validação
completa de repetição, suspensão e recuperação ainda está pendente.
Macros executáveis não são suportadas. O teste de kernel
isolado encontrou EACCES nos novos nós virtuais. A rodada física do Logitech
G PRO Wireless confirmou atalhos, pausa/reativação, troca de perfil com botão
pressionado, desconexão e encerramento/reinício do daemon. Evidências e limites
estão no [registro da rodada física](docs/hardware-validation.md).
Veja [escopo e validação do backend](docs/native-input.md).

## Compatibilidade de dispositivos

O BindForge oferece **remapeamento experimental de mouses e teclados compatíveis
no KDE Wayland, com modelos validados**. A implementação usa interfaces padrão de
entrada do Linux e não depende de uma marca específica, mas isso não garante
compatibilidade com qualquer modelo ou com todas as funções de um periférico.

O ambiente alvo é Fedora KDE Plasma 6 com Wayland. O Logitech G PRO Wireless tem
[uma rodada física documentada](docs/hardware-validation.md); o Aula F99 tem
confirmação de remapeamento básico. Outros modelos e ambientes exigem validação.
Windows e macOS ainda não são suportados.

Veja os [critérios e limites de compatibilidade](docs/devices.md#compatibilidade-de-remapeamento),
incluindo botões adicionais, teclas especiais, layouts e a diferença entre
autorizar acesso e oferecer suporte a uma função.

## Estrutura e arquitetura

```text
apps/desktop/               React + TypeScript + Vite; shell Tauri 2 opcional
crates/remap-core/          modelos e política portáveis; serde opcional
crates/remap-config/        configuração versionada, validação e gravação atômica
crates/remap-ipc/           contrato de estado versionado e transporte D-Bus opcional
crates/platform-kde/        tradução KWin e monitor D-Bus de sessão opcional
crates/platform-linux/      metadados de dispositivos e identidade Steam do Gamescope
crates/bindforge-daemon/    processo separado; monitor e remapeamento nativo/simulado
crates/bindforge-access/    política de acesso e helper administrativo por polkit
integrations/kwin/          fonte da ponte temporária incorporada ao daemon
examples/focus-events.txt   eventos sintéticos para reprodução
```

Fluxo implementado: `stdin/demo -> WindowMonitor -> ProfileSwitcher -> DryRunRemapper`.
A UI Tauri chama o core para uma prévia e consulta `Status1.GetStatus` no daemon
para o painel de estado. O painel e a prévia são independentes.
Com a feature `kde-dbus`: `KWin -> D-Bus de sessão -> daemon -> DryRunRemapper`.
No modo `--kde`, o backend experimental aplica as ações ao mouse e ao teclado após ativação
explícita de cada um; enquanto pausado, a seleção de perfis continua sem remapear entrada.

O core define `Device`, `DeviceKind`, `Binding`, `InputControl`, `Action`, `Profile`,
`AppRule`, `ActiveWindow` e as abstrações `WindowMonitor` e `InputRemapper`.
IDs de dispositivo são fornecidos pelo adaptador. O backend Linux traduz nomes
de teclas para códigos físicos evdev e valida ações e capacidades da interface
antes de aplicar o mapa. Não existe ação de execução de shell.

`WindowMonitor` diferencia ausência de janela de fim de stream. O `ProfileSwitcher`
aplica apenas mudanças de perfil e atualiza seu estado após sucesso. Falhas são
propagadas e podem ser tentadas novamente. `InputRemapper` exige substituição atômica
e reset; há implementações de simulação e do backend experimental de entrada Linux.

O desktop tem seu próprio manifest Cargo e está **excluído do workspace raiz** para
que testes do core/daemon não exijam GTK/WebKit ou acesso à rede. Ele usa o mesmo
`remap-core` por dependência de caminho, sem copiar o algoritmo de seleção.

## Rodar o core e o daemon

Requer Rust/Cargo **1.87+** para o workspace atual (core isolado: MSRV declarado 1.82).
O core/daemon não precisam de bibliotecas gráficas. A enumeração Linux usa libudev;
para compilar esse adaptador no Fedora, instale `systemd-devel` se estiver ausente.
A interface e o daemon rodam como usuário comum. Somente o helper de autorização
executa uma operação administrativa curta via polkit; veja [acesso aos dispositivos](docs/device-access.md). No primeiro checkout, execute
`cargo fetch --locked` para preencher o cache de resolução; depois os comandos
abaixo podem usar `--offline`. A feature D-Bus adiciona zbus. O core isolado não tem dependências obrigatórias;
a persistência habilita sua feature opcional serde.
Na pasta raiz:

```bash
cargo test --workspace --offline
cargo run -p bindforge-daemon --offline
cargo run -p bindforge-daemon --offline -- --stdin < examples/focus-events.txt
```

A demonstração termina automaticamente. A saída esperada é:

```text
simulation profile=default
simulation profile=firefox
simulation profile=konsole
simulation profile=default
```

`--stdin` aceita um ID exato por linha; linha vazia significa ausência de foco.
EOF encerra e faz reset. O limite é 4096 bytes por registro, incluindo newline;
UTF-8 inválido e caracteres de controle são rejeitados. Entrada inválida encerra
com erro após reset. Pela linha de comando, o daemon roda em primeiro plano.
O aplicativo também pode gerenciá-lo como serviço do usuário, conforme a seção abaixo.
No modo `--kde`, SIGINT e SIGTERM encerram a captura e removem a ponte temporária.
**Não execute a UI nem o daemon como root.**

Para desenvolvimento, quando rustfmt e Clippy estiverem instalados:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --offline -- -D warnings
```

No toolchain empacotado pelo Fedora, os componentes costumam ser fornecidos por
`rustfmt` e `clippy`; no rustup, por `rustup component add rustfmt clippy`.

## Regras de seleção

- Predicados de uma regra são AND; regras de um perfil são OR.
- App ID e caminho do executável são comparações exatas sensíveis a maiúsculas.
  Título usa substring, também sensível a maiúsculas. Metadado ausente não satisfaz
  um predicado preenchido. O protótipo KDE não coleta títulos ou executáveis.
- Maior prioridade vence, inclusive prioridades negativas. Em empate, vence a maior
  quantidade de predicados da regra **que casou**, seguida do menor ID em ordem lexical.
- Perfis desabilitados são ignorados. Sem correspondência/foco usa o fallback.
  A prioridade do fallback não participa da competição.
- IDs vazios/duplicados, regras vazias, predicados em branco e fallback ausente,
  desabilitado ou com regras são rejeitados na construção do conjunto.

Os exemplos ficam em `demo_profiles()` e são a configuração inicial quando ainda não há arquivo salvo. O modo `--kde` e a prévia nativa usam os perfis persistidos; a demo finita e `--stdin` continuam com os exemplos fixos.
`firefox` é apenas um ID de demonstração: instalações Flatpak e metadados KWin podem
fornecer outros IDs. Consulte os valores observados e configure regras explícitas.

## Frontend React e shell Tauri 2

Para trabalhar sem reinstalar o RPM, veja o [guia de desenvolvimento local](docs/desenvolvimento.md), com os comandos da interface e do daemon.

Requer Node 22.6+ e npm (os testes usam remoção nativa de tipos TypeScript). Na pasta raiz:

```bash
cd apps/desktop
npm ci
npm run dev
```

Abra http://127.0.0.1:1420. No navegador, a interface mostra os limites do protótipo;
as consultas ao core e ao daemon só são habilitadas no shell nativo. Nele, o
inventário apresenta dispositivos reais e o remapeamento depende da ativação
explícita de mouse ou teclado cadastrado. O navegador não remapeia entrada.

```bash
npm run build
npm run tauri dev
# verificar o Rust do shell, com as dependências de sistema presentes:
cargo check --manifest-path src-tauri/Cargo.toml
# compilação local do executável, sem instalador:
npm run tauri build -- --no-bundle
```

No Fedora tradicional, a documentação oficial do Tauri lista dependências como:

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install "c-development"
```

Isso é preparação de ferramentas de desenvolvimento; a aplicação sempre roda como
usuário comum. Consulte as [instruções oficiais do Tauri](https://v2.tauri.app/start/prerequisites/)
para seu Fedora, especialmente Kinoite/OSTree. Nenhum pacote do sistema é instalado
por este projeto. O shell não tem plugins genéricos de shell/processo/filesystem. Comandos tipados
oferecem prévia, perfis, inventário, ativação/pausa de mouse e teclado e gerenciamento do
serviço do usuário. A assinatura de estado usa sinais D-Bus e um canal Tauri;
o estado não é consultado periodicamente pela interface.

## Serviço do usuário e pacote Fedora

Em **Início → Serviço do BindForge**, use **Iniciar serviço**, **Parar serviço**
ou **Reiniciar serviço**. Não é necessário abrir um terminal. O serviço acompanha
sua sessão gráfica e continua funcionando depois que a janela é fechada.
O ícone do BindForge permanece na bandeja do sistema enquanto a interface estiver
em execução, inclusive com a janela aberta. Fechar a janela apenas a oculta;
use **Abrir BindForge** no menu do ícone para restaurá-la. O menu também oferece
**Iniciar remapeamento do mouse/teclado**, com os cadastros correspondentes para
escolher, e **Pausar remapeamento do mouse/teclado**. **Sair da interface** encerra somente a interface e seu ícone,
mantendo o daemon em execução. Para encerrar o daemon, use **Parar serviço**.
Iniciar, reiniciar e retornar de bloqueio/suspensão mantêm a captura pausada até
ativação explícita de cada dispositivo.

**Iniciar serviço ao entrar no KDE** é opcional e vem desativado. Marcar prepara
o próximo login sem iniciar o serviço agora; desmarcar não para uma execução atual.
Um daemon iniciado pelo terminal é identificado como externo e não é encerrado pelo
app: encerre-o na origem para passar a usar o serviço. Configurações personalizadas
com o mesmo nome não são sobrescritas. O app gerencia somente
`bindforge-daemon.service` no systemd do usuário.

Para construir o RPM contendo a interface, o daemon e a unidade de serviço:

```bash
cd apps/desktop
npm ci
npm run package:fedora
```

O resultado fica em `target/release/bundle/rpm/` na raiz. O pacote tem entrada no
menu de aplicativos e inclui o daemon em `/usr/libexec/bindforge/bindforge-daemon`.
A geração do pacote não o instala nem ativa serviços. O pacote atual é destinado
a Fedora 44 x86_64/KDE; instalação e login reais ainda precisam de validação.
Veja [instalação e ciclo do serviço](docs/desktop-service.md).

## Início e editor visual do mouse

As abas são **Início**, **Perfis** e **Dispositivos**. Início é a tela inicial:
cadastre seus periféricos em **Dispositivos → +**, escolha o perfil no dropdown
**Editar perfil** e clique no cartão do mouse cadastrado.
O indicador **Ativo no daemon** é independente dessa seleção e continua seguindo
o foco da janela. Os desenhos são genéricos, sem associação a um fabricante.

No modal do mouse, selecione **Todos os mouses compatíveis** ou uma interface
identificada pelo daemon. O cartão já abre a interface cadastrada. A lista usa a
quantidade de botões informada (3 a 32); o desenho mostra até cinco controles
básicos. Os botões extras ficam na lista. O modal de ação oferece tecla ou
atalho, outro botão do mouse, desativação e comportamento original/geral.
**Aplicar ao rascunho** atualiza o editor; **Salvar alterações** grava o perfil.
Fechar com mudanças pendentes pede confirmação. A troca de abas mantém o perfil
em edição escolhido; as alterações salvas ficam disponíveis nas duas telas.

Ações específicas de um dispositivo substituem a ação geral para o mesmo controle.
Remover uma ação específica volta à geral; remover a geral restaura a função
original, mantendo eventuais exceções específicas. O mapa de cada perfil continua
independente. O editor do mouse aceita nomes simbólicos para as teclas de destino;
o editor de teclado usa códigos físicos no rascunho.

**Salvar edita a configuração. A entrada física só é capturada após ativação explícita.**
O teclado abre um editor com teclas selecionáveis para ABNT2, ANSI e ISO.
Escolha a tecla, o destino e modificadores opcionais; as alterações são salvas por
interface e perfil. O desenho preserva as proporções com rolagem horizontal em
telas estreitas. Ative a captura física em **Início → Remapeamento do teclado**.
A identificação de dispositivos continua sendo de interfaces, com IDs locais à
conexão, conforme [docs/devices.md](docs/devices.md).

## Autorizar acesso aos dispositivos

Em **Dispositivos → editar cadastro → Gerenciar acesso**, consulte as permissões,
confira a interface detectada e clique em **Conceder acesso**. A janela de autenticação
do sistema autoriza somente a criação de regras específicas e o acesso à saída virtual;
a interface e o daemon continuam como usuário comum. O mesmo botão está nos painéis
de remapeamento em Início. Salvar perfis e instalar o RPM não ativam a captura.

**Remover autorização** pausa a captura correspondente e remove apenas regras
criadas pelo gerenciador, bloqueando novas ativações até uma nova concessão.
Esse bloqueio vale mesmo com ACL residual ou permissão externa. Reconectar o
dispositivo encerra acessos residuais do sistema; permissões externas podem permanecer. Sem série individual, a regra fica vinculada
à porta/interface, conforme indicado na tela. Autorize cada conexão vinculada ao
cadastro. Sem série individual, trocar de porta pode exigir um novo vínculo. O fluxo exige o helper e a política incluídos no RPM.
Veja [arquitetura, instalação e limites](docs/device-access.md).

## Remapeamento experimental do teclado

Depois de salvar o mapa no editor, abra **Início → Remapeamento do teclado**,
selecione o teclado cadastrado, solte todas as teclas e clique em **Ativar
remapeamento**. O mapa aplicado é o do perfil ativo no daemon, conforme o foco;
o perfil em edição continua independente. **Pausar remapeamento** restaura a entrada
original. A bandeja também oferece ativação e pausa de teclado.

Esta etapa aceita uma interface alfanumérica física por vez. Mouse e teclado podem
ficar ativos simultaneamente quando usam interfaces distintas. Se ambos os cadastros
apontarem para a mesma interface composta, pause um antes de ativar o outro.
São suportados tecla → tecla, atalho, botão do mouse e desativação. Teclas sem ação
passam normalmente; o motor transforma a repetição física no destino do mapa.
**Ctrl + Alt + Pause** no teclado físico pausa o teclado por emergência, mesmo que
essas teclas tenham ações personalizadas. Bloqueio, suspensão, perda de foco,
desconexão e encerramento liberam a captura; o retorno exige ativação explícita.

Reinicie interface e daemon juntos para habilitar os novos comandos. Esta etapa
passou nos testes automatizados do motor/IPC e na interface com API simulada.
O teste virtual `--keyboard-self-test` parou por EACCES ao abrir a fixture, antes de
emitir eventos. O remapeamento básico no Aula F99 foi confirmado manualmente;
repetição no KDE, indicadores de lock e recuperação ainda precisam de validação física. Veja [escopo e validação](docs/native-input.md).

## Remapeamento experimental do mouse

Em **Início → Remapeamento do mouse**, selecione um cadastro salvo e clique em
**Ativar remapeamento**. Pause outros remapeadores nesse mouse antes de ativar.
**Pausar remapeamento** restaura seu funcionamento original. O daemon sempre inicia
pausado. Fechar somente a UI mantém o daemon funcionando; use **Parar serviço**
para encerrar o serviço ou Ctrl+C quando ele foi iniciado no terminal.

A primeira etapa aceita uma interface física com funções de mouse por vez,
incluindo interfaces compostas compatíveis. Teclados sem ponteiro e dispositivos
virtuais são recusados. O perfil ativo vem do
foco da janela; o dropdown Editar perfil continua independente dessa seleção.
Teclas/atalhos, outros botões e desativação são suportados para os códigos de mouse
implementados. Interfaces sem permissão ou já capturadas retornam um erro visível.
Permissões podem ser concedidas pelo fluxo administrativo explícito de Gerenciar acesso.
Não há inclusão em grupos nem concessão automática ao ativar.

**Validação física do Logitech G PRO Wireless:** atalhos laterais, pausa/reativação,
troca de perfil com botão pressionado, desconexão e encerramento/reinício passaram
com o daemon como usuário comum. Bloqueio de sessão e suspensão agora pausam a
captura via logind e exigem nova ativação ao retornar. Os testes isolados e a
validação física dessa etapa passaram. Veja [ciclo da sessão](docs/session-lifecycle.md).
O self-test de kernel como usuário comum parou antes de emitir eventos por falta
de acesso ao nó virtual recém-criado. O backend continua experimental; resultados
e pendências estão no [registro da rodada física](docs/hardware-validation.md).

## Integração KDE automática

Inicie o daemon com `--kde`: ele carrega a ponte temporária no KWin, recebe o foco
inicial e acompanha as trocas de janela. Não é necessário instalar ou habilitar
um script nas configurações do KDE. A ponte é incorporada ao executável e funciona
sem o checkout do projeto. Se o KWin reiniciar ou a ponte for descarregada, o
daemon tenta reconectar automaticamente.

Em **Início → Integração com KDE → Ver integração com KDE**, confira a conexão,
recebimento de foco e migração anterior. **Reconectar ao KDE** pede ao daemon para
recarregar sua ponte; **Verificar novamente** atualiza o diagnóstico. A UI não instala
scripts nem inicia o daemon.

A instalação persistente da versão anterior é desabilitada e descarregada
na primeira conexão, evitando duas pontes. Seu pacote é mantido no disco; as demais
preferências do KDE são preservadas. Somente essa migração usa `kreadconfig6` e
`kwriteconfig6`; o funcionamento normal dispensa kpackagetool6.

Ctrl+C e SIGTERM encerram o daemon, restauram a simulação e descarregam a ponte,
removendo o arquivo temporário. SIGKILL ou falha abrupta não permitem limpeza imediata;
a próxima execução recupera a ponte remanescente pelo nome reservado. Fora do KDE,
o daemon aguarda o KWin e o diagnóstico informa a indisponibilidade.

## Criar e editar perfis

No aplicativo nativo, abra **Seus perfis → Criar perfil**. A criação e a edição
abrem em modais centralizados, com o fundo bloqueado. Informe nome e IDs dos
aplicativos (um por linha), escolha copiar ações de outro perfil ou começar sem
remapeamentos e salve. A cópia é independente. A tela também permite renomear,
alterar associações, habilitar/desabilitar e ajustar prioridade, com aviso de
sobreposição. O botão **Excluir** em cada perfil específico abre um modal de confirmação e salva a remoção;
o daemon reavalia a seleção automaticamente. O Padrão não pode ser excluído e
permanece habilitado e sem regras. Regras avançadas já
existentes são preservadas e suas associações não são editáveis nesta tela.

Os perfis são gravados em `$XDG_CONFIG_HOME/bindforge/profiles.json`, ou
`~/.config/bindforge/profiles.json` quando XDG não está definido. Não é
criado arquivo até o primeiro salvamento. A prévia **Explorar a seleção de perfis**
consulta a configuração salva. O botão **Detectar pela janela** abre um modal de
captura: aguarde **Captura pronta**, clique na janela desejada ou use Alt+Tab e volte
para o formulário. O ID é acrescentado à lista sem duplicatas. Você pode cancelar
ou capturar outra janela. O prazo é de 30 segundos; requer daemon `--kde` com sua ponte
temporária conectada ao KWin. Reinicie o daemon e a interface uma vez após atualizar para
habilitar os novos comandos. A detecção preenche o formulário; só **Salvar alterações**
grava a associação. A aba **Início** oferece o editor visual do mouse; ações existentes são preservadas
ao editar os dados do perfil. O editor visual do teclado salva as ações; a execução física depende da ativação explícita do teclado no daemon.

O daemon `--kde` carrega o mesmo arquivo e verifica alterações durante sua espera
de foco (intervalo de 500 ms, além do tempo de consulta D-Bus). Uma gravação válida
reavalia a última janela observada, mesmo sem uma nova troca de foco, e publica o
resultado por evento. A aplicação física depende de o dispositivo estar explicitamente ativo. Se o arquivo ficar inválido durante
a execução, o daemon mantém a última política válida e registra um erro; arquivo
inválido no início impede a inicialização. Reinicie daemon e interface uma vez
para usar esta versão; depois, editar perfis não exige reinício.

Jogos Steam iniciados por uma instância Gamescope própria podem ser identificados
como `steam:<AppId>`, por exemplo `steam:1808500`. O daemon consulta apenas o
`SteamAppId` desse processo ou o marcador explícito `SteamLaunch AppId=...` do
comando filho da Steam (incluindo `steam-launch-wrapper --`), usando o PID informado
pelo KWin, sem procurar jogos em outros processos. A resolução também funciona
quando o KWin fornece o PID, mas deixa os IDs da janela vazios. Se a identificação não estiver disponível, a captura avisa e
não acrescenta `gamescope` como se fosse um jogo específico. Associações manuais a
`gamescope` recebem um aviso e correspondem apenas aos casos não resolvidos.
A ponte incluída no daemon é usada automaticamente ao reiniciá-lo. Não é necessário alterar as opções gráficas do jogo.

O formato e os limites estão em [docs/profiles.md](docs/profiles.md).

## Consultar o estado do daemon pela interface

Inicie o daemon na raiz, em um terminal:

```bash
cargo run -p bindforge-daemon --features kde-dbus -- --kde
```

Em outro terminal:

```bash
cd apps/desktop
npm ci
npm run tauri dev
```

O painel **Perfil no daemon** recebe um snapshot ao conectar e depois atualizações
por evento, sem a espera programada de um segundo. Ao voltar para a janela deste app, o perfil atual
normalmente retorna a `default`. **Últimas trocas de perfil** começa recolhido; use o toggle **Exibir histórico**
para mostrar ou ocultar a lista. A coleta continua enquanto o histórico está oculto.
O histórico mantém as 12 últimas
seleções diferentes consumidas pelo daemon, em memória, com a mais recente no topo.
Isso permite conferir uma passagem pelo Konsole mesmo depois de voltar ao app.
Após atualizar esta versão, reinicie o daemon e a interface para habilitar os sinais.
Um daemon antigo gera um aviso de reinício; a UI não volta ao polling silenciosamente.
A consulta de diagnóstico continua compatível com as versões anteriores. Exibe o ID do perfil selecionado, o estado do monitor
e uma revisão de atualização, com indicação do modo pausado/ativo. Antes do primeiro evento KWin,
o monitor aparece como aguardando foco mesmo com o perfil padrão selecionado.
O daemon carrega sua ponte automaticamente para fornecer foco real.

A saída do daemon retira o snapshot antigo da tela e mostra indisponibilidade.
O painel observa a chegada de uma nova instância no bus e recebe seu snapshot,
aceitando a revisão reiniciada do processo. A prévia de perfis não altera o daemon. A UI salva perfis no arquivo do usuário. O daemon carrega a ponte temporária e a UI oferece diagnóstico e reconexão.
O daemon continua sendo iniciado separadamente.

O cliente Rust, a API, o fluxo por eventos, a reconexão e a preservação do histórico
foram testados em bus D-Bus isolado. O shell Tauri passou em `cargo check` com os pré-requisitos agora
presentes neste ambiente; o usuário confirmou a conexão da interface nativa.
A inspeção visual do novo fluxo por eventos requer reiniciar o daemon em execução.
Não foi medida a latência total no desktop; a espera periódica de um segundo foi removida.
No navegador, o painel mostra que a consulta está disponível somente no nativo.

O protocolo está descrito em [docs/status-ipc.md](docs/status-ipc.md). Para testes:

```bash
cargo test --workspace --all-features --locked
cd apps/desktop
npm test
npm run build
```

## Observar foco real no KDE (entrada inicia pausada)

```bash
cargo run -p bindforge-daemon --features kde-dbus -- --kde
```

O daemon carrega a ponte automaticamente e recebe o foco inicial e mudanças
seguintes. Ao reiniciar o daemon ou o KWin, a ponte é carregada novamente, sem
habilitação manual nas configurações do KDE.
O receptor aceita `FocusChanged` e `FocusChangedWithProcess` somente do dono atual de `org.kde.KWin`.
A perda/troca desse dono retorna ao fallback e inicia a recuperação da ponte.

Para testar a ponte sem tocar no KDE, requer `dbus-daemon` no PATH:

```bash
cargo test --workspace --all-features --locked
node --test integrations/kwin/tests/focus.test.cjs
```

O teste cria um bus privado e simula o dono de KWin; não usa o bus da sessão real.
Em sandboxes, pode ser necessário permitir criação de sockets locais para o teste.

## Identificar mouses e teclados

Após atualizar, reinicie o daemon e a interface com os mesmos comandos acima.
Na aba **Dispositivos**, clique no cartão **+ Configurar dispositivo**. Informe
nome, tipo (mouse/teclado), quantidade de botões ou layout físico (ABNT2, ANSI, ISO)
e a interface correspondente. O cadastro é salvo junto aos perfis e compartilhado
entre eles. Clique no cartão cadastrado para editar seus dados.

A lista técnica fica recolhida em **Interfaces identificadas**, com nome, tipo,
conexão e IDs de fabricante/produto quando disponíveis.

A lista representa **interfaces**, não necessariamente periféricos físicos distintos:
um receptor pode oferecer teclado e mouse separados. Interfaces compostas aparecem
como “Mouse e teclado”. Entradas de software conhecidas aparecem como “Virtual”.

Em **Editar dispositivo**, vincule receptor, cabo e Bluetooth como conexões do
mesmo cadastro. Cada conexão recebe um nome; **Priorizar** determina qual será
usada quando várias estiverem presentes. Priorize o cabo para usá-lo com o
receptor ainda conectado. As ações são compartilhadas e preservadas em todos os
perfis, inclusive na migração de cadastros antigos.

Após ativação explícita, o daemon acompanha as conexões automaticamente e troca
para a primeira disponível. Se todas desconectarem, aguarda uma conexão vinculada.
Pausa manual, bloqueio, suspensão e falhas de captura exigem nova ativação.
O reconhecimento usa série/endereço físico quando disponível; sem identificação
individual, pode depender da porta. A lista técnica consulta somente metadados e
pode ser renovada por **Atualizar lista** ou **Atualizar interfaces** no cadastro.
Veja [conexões do mesmo dispositivo](docs/devices.md#conexões-do-mesmo-dispositivo).

Também é possível testar sem interface ou KDE:

```bash
cargo run -p bindforge-daemon --features linux-devices -- --list-devices
```

O adaptador usa libudev; no Fedora, a dependência de compilação é `systemd-devel`.
O inventário não concede acesso a `/dev/input` nem altera grupos. A concessão de
regras udev exige Gerenciar acesso e autenticação administrativa; a captura usa as
permissões já disponíveis.
Detalhes e limites de identidade estão em [docs/devices.md](docs/devices.md).

## Teste rápido na sua sessão KDE

Requer Python 3, `gdbus`, `kreadconfig6` e um Plasma já em execução. Este teste é opt-in: carrega
automaticamente a ponte temporária no compositor da sessão atual, sem instalar pacotes,
alterar configurações ou pedir ativação de janelas.

```bash
cargo build -p bindforge-daemon --features kde-dbus --locked
python3 tools/test-kde-session.py
# Também validar a API de estado após o snapshot real:
python3 tools/test-kde-session.py --check-status
```

O teste inicia seu próprio daemon, verifica a rejeição de um cliente comum e
aguarda o snapshot inicial da ponte carregada automaticamente.
Encerra esse daemon com SIGTERM e verifica o unload da ponte. Não registra títulos
ou IDs de aplicativo.
Se já existir um daemon com o nome de serviço, o teste recusa execução.

A versão anterior do smoke foi executada em Fedora 44, Wayland, KWin 6.7.5.
O ciclo automático atual foi validado em bus privado, incluindo encerramento por
sinais; o smoke atual não foi repetido na sessão do usuário. O teste anterior validou o caminho
real de foco inicial; não valida ainda a troca entre aplicativos e entrada física.
O daemon também oferece `--kde --once`, que termina após um evento de foco e faz
reset da simulação; sozinho, esse modo não tem timeout. O teste Python impõe prazo.

## KDE Wayland e expansão

Veja [o plano e os limites de segurança](docs/kde-wayland.md). O script experimental
usa `workspace.windowActivated` e `workspace.activeWindow`, descritos na
[API oficial do KWin](https://develop.kde.org/docs/plasma/kwin/api/).
Ele envia IDs de aplicativo e o PID da janela ao receptor; não registra títulos, PIDs ou IDs em logs.
O snapshot inicial, a troca entre janelas e o remapeamento do Logitech G PRO
Wireless foram validados no KWin 6.7.5 real. Bloqueio de sessão e suspensão
pausam a captura; SIGINT/SIGTERM liberam a entrada e descarregam a ponte.
Veja os registros de [hardware](docs/hardware-validation.md) e
[sessão](docs/session-lifecycle.md). Controle de taxa específico para o receptor
de foco continua pendente.

Adaptadores futuros deverão implementar as mesmas abstrações, com dependências
específicas em crates separados/`cfg(target_os)`. Windows precisará de decisões sobre
observação de foco, Raw Input/hooks e injeção; macOS, sobre NSWorkspace/Accessibility
e permissões de entrada. Nenhuma dessas opções foi implementada ou validada.

## Validação

- `cargo test --workspace --offline`: testes de regras, prioridades, fallback,
  falhas/repetição, tradução KWin e protocolo de entrada.
- Demo do daemon executada com a sequência esperada.
- O estado das verificações do frontend está registrado em [docs/validation.md](docs/validation.md).
- `cargo fmt` e `cargo clippy` indisponíveis no ambiente de implementação.
- A ponte passou no bus privado e no teste de snapshot inicial com KWin real.
  O usuário confirmou trocas de perfil nos logs do daemon e conexão no shell nativo.
  As validações físicas e de ciclo de sessão estão registradas nos documentos acima.

Próximas etapas: validar instalação/login do pacote na sessão real, medir latência,
ampliar a cobertura de hardware, adicionar controle de taxa do receptor de foco
e completar a validação física do backend de teclado.

A marca, os executáveis e o pacote agora são **BindForge**. Perfis anteriores
são migrados automaticamente para a pasta `bindforge`, preservando o original.
Atualize a interface e o daemon juntos; a versão nova recusa iniciar ao lado
de um daemon da versão anterior. Referências antigas no código são restritas
à migração e detecção de compatibilidade.

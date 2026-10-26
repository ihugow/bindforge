# Inventário de mouses e teclados no Linux

## Estado e limites

O inventário identifica interfaces de mouse/teclado e as apresenta na UI sem abrir
fluxos de eventos. O backend experimental de captura é separado e só inicia por
ativação explícita; veja [native-input.md](native-input.md). A consulta do inventário
não exige autorização extra, não abre fluxos de eventos nem instala regras udev.
A autorização para remapear é um fluxo separado, descrito em
[acesso aos dispositivos](device-access.md).
O adaptador usa libudev e atributos sysfs apenas para consulta de metadados.

A UI consulta snapshots ao conectar ao daemon, periodicamente e pelo botão
**Atualizar lista**. O daemon acompanha conexões de dispositivos ativados com
consultas de metadados a cada 500 ms (mais o tempo de processamento do loop).
O cadastro aceita múltiplas conexões explícitas com identidade persistente quando
os metadados permitem; veja o fluxo de conexões abaixo.

## Arquitetura

`React → list_devices (Tauri) → Devices1.GetDevices → platform-linux → libudev/sysfs`.

O core define `DeviceEnumerator`, `Device`, `DeviceKind` e `DeviceConnection`, sem
bibliotecas de SO. `platform-linux::LinuxDeviceEnumerator` implementa a abstração.
A interface D-Bus roda a enumeração em `blocking::unblock`, fora do executor de
mensagens do daemon. A UI nativa apenas consulta o daemon em tarefa bloqueante
separada do loop gráfico. Não recebe parâmetros de caminho de dispositivo/endereço.

O comando de terminal é independente do KDE e enumera como usuário comum:

```bash
cargo run -p bindforge-daemon --features linux-devices -- --list-devices
```

Para compilar no Fedora, são necessários libudev/pkg-config, normalmente fornecidos
por `systemd-devel` e `pkgconf-pkg-config`. Já estavam presentes no ambiente testado;
nenhum pacote de sistema foi instalado nesta etapa.

## Classificação

São enumerados nós `event*` inicializados do subsistema `input`. A classificação usa
`ID_INPUT_MOUSE` e `ID_INPUT_KEYBOARD` do udev. Ambas as flags resultam em interface
composta, sem presumir que existam dois periféricos físicos. Touchpads/touchscreens
não são classificados como mouse; interfaces de teclado continuam visíveis quando
a outra função é um touchpad. Nós genéricos com apenas `ID_INPUT_KEY` não viram
teclados: isso evita incluir botões de energia e controles multimídia como teclados
completos. Um periférico que exponha somente controles especiais pode não aparecer.

A conexão vem de `ID_BUS`: USB, Bluetooth, integrado conhecido ou desconhecido.
O caminho `/sys/devices/virtual/input/` identifica entradas virtuais conhecidas. Isso
não detecta todas as formas possíveis de emulação; Bluetooth via UHID não é marcado
como virtual somente por residir sob `/sys/devices/virtual/misc`.

Nome e IDs de fabricante/produto vêm de propriedades/atributos. Nomes são limitados
a 256 caracteres, sem controles. IDs hexadecimais ausentes/inválidos são desconhecidos,
não valores fabricados. Números de série não são coletados pelo inventário; o
helper de autorização pode usá-los para delimitar uma regra de acesso. A ordenação usa nome e ID.

## Identidade e duplicatas

Cada interface mantém um ID opaco ao frontend baseado no syspath atual. Esse ID
pode mudar ao reiniciar ou reconectar. Os novos vínculos salvam também a impressão
digital do seletor udev usado pelas autorizações: fabricante/produto, interface e
número de série/endereço físico quando disponível; sem isso, usam a porta física.
A resolução exige correspondência única, sem agrupar pelo nome ou modelo.
Sem número de série, trocar de porta pode exigir um novo vínculo.
Cadastros antigos sem `connections` mantêm o comportamento por syspath até serem
editados e salvos; vínculos antigos desconectados podem ser preservados sem
impressão digital, sem prometer reconhecimento persistente.

Duas interfaces com o mesmo nome continuam separadas. Não agrupamos pelo receptor
USB, pois um mesmo receptor pode representar mouse e teclado distintos. Um mouse
gamer pode expor funções de teclado e aparecer como composto. A contagem na UI é
de interfaces, não de periféricos físicos. Não existe promessa de compatibilidade
de remapeamento, firmware, DPI ou RGB a partir de uma entrada aparecer na lista.

## Compatibilidade de remapeamento

A implementação é genérica: usa interfaces padrão de entrada do Linux
(evdev/uinput), em vez de um driver próprio para cada marca. Um modelo pode
funcionar sem uma integração específica, desde que exponha os eventos e as
capacidades aceitos pelo backend. **Não há garantia de suporte universal.**
Aparecer no inventário ou receber autorização de acesso não comprova que todas
as funções do dispositivo possam ser remapeadas.

- **Ambiente:** o alvo é Fedora KDE Plasma 6 em Wayland. Outros ambientes Linux
  ainda exigem validação; Windows e macOS não têm backend implementado.
- **Mouse:** são remapeáveis os oito códigos de botão atualmente suportados:
  esquerdo, direito, central e os códigos simbólicos 8 a 12. O cadastro permitir
  declarar até 32 botões não amplia essa capacidade do backend. Botões extras de
  fabricantes podem emitir outros eventos ou depender de protocolos próprios.
- **Teclado:** o remapeamento usa códigos de teclas físicos Linux. O editor oferece
  ANSI 104, ISO 105 e ABNT2 107; ele não altera o idioma/layout configurado no
  sistema. Teclas Fn, macros de hardware, teclas especiais e funções exclusivas do
  fabricante podem não gerar eventos utilizáveis ou não estar disponíveis no editor.
- **Conexão:** USB, receptor sem fio ou Bluetooth não garantem, por si só,
  compatibilidade. Cada interface precisa satisfazer a validação do backend;
  nem todo tipo de conexão foi testado fisicamente.
- **Funções do fabricante:** ajuste de DPI, RGB, atualização de firmware e
  programação da memória interna não estão implementados.

O Logitech G PRO Wireless tem [validação física documentada](hardware-validation.md)
para atalhos, pausa, troca de perfil e recuperação. O Aula F99 tem confirmação
manual de remapeamento básico; isso não equivale à validação completa de repetição,
bloqueio/suspensão e recuperação. Esses resultados não abrangem outros modelos.

A autorização persistente resolve o acesso a uma interface compatível. Ela não
adiciona suporte a um botão, tecla ou protocolo ausente. Os detalhes técnicos e as
restrições de captura estão em [backend de entrada](native-input.md).

## Contrato D-Bus

Serviço `org.bindforge.Daemon`, caminho `/org/bindforge/Daemon`, interface
`org.bindforge.Daemon.Devices1`, método `GetDevices() -> (u, a(ssssii))`.

A versão é 1. Cada linha contém ID, nome, tipo, conexão, fabricante e produto.
Tipos: mouse, keyboard, composite. Conexões: usb, bluetooth, internal, virtual, unknown.
Fabricante/produto são inteiros entre 0 e 65535; -1 representa valor desconhecido.
O DTO Tauri os expõe como número ou null. A lista é limitada a 256 interfaces e
metadados inválidos/IDs duplicados são rejeitados, sem devolver uma lista parcial
como se fosse completa. A consulta usa o dono único do daemon e timeout de 3 segundos
por chamada. A ausência dessa API em um daemon antigo gera instrução de reinício.

Uma falha é diferente de uma consulta bem-sucedida vazia. A UI remove resultados
antigos ao desconectar/recarregar e ignora respostas de consultas já descartadas.
Não há escrita de configurações ou ações de entrada nessa interface.

## Verificação

- Enumeração real no Fedora 44 retornou 8 interfaces, incluindo USB e entradas
  virtuais do keymasq. Não houve desconexão física ou teste automático de hotplug.
- Testes cobrem tipos, exclusão de touchpad, virtual/Bluetooth, nomes e IDs inválidos.
- Testes do protocolo cobrem lista vazia, interfaces distintas com mesmo nome,
  IDs duplicados, limite da lista e versão inválida.
- A integração D-Bus executa um daemon real em bus privado, consulta os metadados
  locais e verifica que isso não muda perfil/revisão. Um serviço legado recebe erro
  de atualização necessário, em vez de aparecer como lista vazia.
- Dois testes JS cobrem lista vazia versus erro e descarte de resposta atrasada.
- A fixture `tests/devices-browser.html` renderiza o componente real com dados
  sintéticos claramente identificados. O layout foi inspecionado no navegador.
- O shell nativo passou em `cargo check`. A nova consulta visual com os dados reais
  no app depende do reinício do daemon do usuário, preservado durante a implementação.

Referências: [udev-rs](https://docs.rs/udev/0.9.3/udev/),
[classificação input_id do systemd](https://github.com/systemd/systemd/blob/main/src/udev/udev-builtin-input_id.c).

## Cadastro de dispositivos

A aba abre com um cartão **+ Configurar dispositivo**, seguido dos dispositivos
cadastrados. O modal recebe nome, tipo, propriedades e uma interface compatível.
Mouses têm de 3 a 32 botões (incluindo clique da roda, excluindo rolagem); teclados
usam layout físico ABNT2, ANSI ou ISO, sem mudar o layout do sistema. O editor de
teclas fica em Início, no cartão do teclado, e salva ações por perfil/interface.
A captura física é ativada separadamente em Início → Remapeamento do teclado;
o remapeamento básico foi confirmado no Aula F99, com validação completa pendente. A lista técnica de interfaces fica em details.

O array opcional `devices` integra a configuração versão 1. Configurações anteriores
sem esse campo carregam como lista vazia. O backend valida até 64 dispositivos,
IDs únicos e um único cadastro por par tipo/interface. Uma interface composta pode
ser associada uma vez a mouse e outra a teclado. Nome e propriedades podem ser
editados sem a interface conectada. Um vínculo novo exige interface compatível
presente no snapshot. Não há agrupamento automático de interfaces por modelo físico.

### Conexões do mesmo dispositivo

Em **Editar dispositivo**, use **Atualizar interfaces**, selecione a interface,
informe um nome (Receptor, Cabo ou Bluetooth) e clique **Vincular conexão**.
Até oito conexões podem pertencer ao mesmo cadastro. A conexão nova entra no topo;
**Priorizar** move uma conexão existente para o topo. Deixe o cabo primeiro para
que seja usado mesmo quando o receptor continuar conectado. Cada cartão mostra o
nome escolhido e, abaixo, o nome da interface detectada. **Editar** abre os campos
para renomear ou trocar a interface; **Salvar conexão** confirma o rascunho e
**Cancelar edição** o descarta. Grave o dispositivo para persistir as alterações.
O nome detectado é preservado para exibição quando a interface estiver desconectada.
O gerenciamento de acesso permite escolher e autorizar cada conexão salva.

As ações dos cadastros com conexões usam `device:<id do cadastro>`, independente
do transporte. Ao salvar um cadastro antigo, suas ações específicas do tipo de
entrada são migradas em todos os perfis. Conflitos entre ações existentes de
interfaces vinculadas são recusados; ações gerais e do outro tipo são preservadas.
Remover ou priorizar uma conexão não apaga nem duplica ações. A gravação mantém
revisão otimista e substituição atômica; reinicie daemon e shell Tauri para usar
os novos campos com binários atualizados.

Após ativação explícita, o daemon usa apenas a primeira conexão disponível e
acompanha sua presença. Ao trocar de transporte, encerra a captura anterior,
libera o estado sintético e abre a nova interface com as mesmas ações e validações
de acesso/capacidades. Se todas desconectarem, mantém a intenção de ativação e
aguarda uma conexão vinculada. Reconexão física pode retomar automaticamente;
pausa manual, mudança de cadastro, perda de foco, bloqueio, suspensão, revogação
e erros de captura cancelam essa intenção e exigem ativação explícita.
Todos os botões/teclas devem estar soltos ao abrir a nova conexão. Uma falha de
autorização ou de abertura pausa o remapeamento, sem tentar outras interfaces
silenciosamente. Não há promessa de transição sem interrupção durante a troca.

Verificação automatizada cobre migração e conflitos, prioridade cabo/receptor,
fallback e reconexão com novo caminho, vínculo lógico do plano, validação da
configuração e respeito às pausas/revogação. A fixture
`tests/connections-browser.html` usa dados sintéticos e permite revisar o formulário.
A troca física entre cabo e receptor ainda precisa de validação no hardware.

No Início, cada cadastro tem cartão com nome, propriedades e estado da interface.
O cartão do mouse abre seu escopo específico e a quantidade declarada de controles;
os botões adicionais aparecem na lista, sem inventar sua posição no desenho genérico.
IDs dos botões continuam simbólicos: 1, 3, 2, 8, 9 e 10 em diante. A quantidade é
informada pelo usuário, sem leitura automática das capacidades ou validação física.
Reduzir a quantidade visível não apaga ações já salvas de outros controles.


## Bateria

Os cards de Início e Dispositivos mostram porcentagem e estado de carga quando
`/sys/class/power_supply` oferece uma bateria com escopo Device, capacidade de
0 a 100 e associação inequívoca à árvore física da interface. Leituras inválidas,
fontes offline/ausentes, interfaces virtuais e associações ambíguas não são usadas.
Não se associa pelo nome ou apenas pelo receptor/modelo; não se estima porcentagem.
Ausência de dados aparece como “Bateria indisponível”.

A consulta é somente leitura, sem sudo, captura de entrada ou comando HID.
`Devices1.GetDeviceBatteries() -> (u, a(sis))` é aditivo ao inventário existente:
versão 1 e linhas com interface, porcentagem e charging/discharging/full/unknown.
O cliente aceita daemons antigos sem esse método, mantendo o inventário e
mostrando bateria indisponível. Os dados não são gravados nos perfis.

A leitura se renova a cada 60 segundos nas abas visíveis, ao voltar à janela e
pelos botões de atualização. Falhas de leitura removem a indicação anterior.
O Logitech G PRO Wireless desta máquina fornece bateria via HID++; o Aula F99
no receptor 2,4 GHz não fornece essa informação pela interface padrão atualmente.


## Gerenciar acesso

No cadastro salvo ou no painel de remapeamento, Gerenciar acesso consulta a entrada,
a saída virtual e a autorização persistente criada pelo BindForge. A concessão e a
remoção usam autenticação administrativa do sistema, com regras derivadas do
dispositivo escolhido, sem regras genéricas por todos os teclados/mouses.
Veja [fluxo e limites das autorizações](device-access.md).

# Inventário de mouses e teclados no Linux

## Estado e limites

O inventário identifica interfaces de mouse/teclado e as apresenta na UI sem abrir
fluxos de eventos. O backend experimental de captura é separado e só inicia por
ativação explícita; veja [native-input.md](native-input.md). Não há autorização extra, abertura de
`/dev/input/event*`, `/dev/uinput`, alteração de grupos ou instalação de regras udev.
O adaptador usa libudev e atributos sysfs apenas para consulta de metadados.

O inventário é um snapshot: carregamento ao conectar ao daemon e botão **Atualizar
lista** para novas consultas. Hotplug automático, identidade persistente e
remapeamento físico ficam para etapas seguintes. O cadastro manual e ações
específicas por interface estão implementados, com o limite de identidade abaixo.

## Arquitetura

`React → list_devices (Tauri) → Devices1.GetDevices → platform-linux → libudev/sysfs`.

O core define `DeviceEnumerator`, `Device`, `DeviceKind` e `DeviceConnection`, sem
bibliotecas de SO. `platform-linux::LinuxDeviceEnumerator` implementa a abstração.
A interface D-Bus roda a enumeração em `blocking::unblock`, fora do executor de
mensagens do daemon. A UI nativa apenas consulta o daemon em tarefa bloqueante
separada do loop gráfico. Não recebe parâmetros de caminho de dispositivo/endereço.

O comando de terminal é independente do KDE e enumera como usuário comum:

```bash
cargo run -p remap-daemon --features linux-devices -- --list-devices
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
não valores fabricados. Números de série não são coletados. A ordenação usa nome e ID.

## Identidade e duplicatas

Cada interface mantém um ID opaco ao frontend baseado no syspath atual. **Esse ID
não é uma identidade persistente**: pode mudar ao reiniciar, trocar de porta ou
reconectar. O vínculo cadastrado é válido para esse identificador e não promete
reconhecimento permanente. Se ele mudar, o cartão indica indisponibilidade e o
usuário pode escolher a nova interface em **Editar dispositivo**.

Duas interfaces com o mesmo nome continuam separadas. Não agrupamos pelo receptor
USB, pois um mesmo receptor pode representar mouse e teclado distintos. Um mouse
gamer pode expor funções de teclado e aparecer como composto. A contagem na UI é
de interfaces, não de periféricos físicos. Não existe promessa de compatibilidade
de remapeamento, firmware, DPI ou RGB a partir de uma entrada aparecer na lista.

## Contrato D-Bus

Serviço `org.appbuttons.Remap`, caminho `/org/appbuttons/Remap`, interface
`org.appbuttons.Remap.Devices1`, método `GetDevices() -> (u, a(ssssii))`.

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
teclas ainda não está implementado. A lista técnica de interfaces fica em details.

O array opcional `devices` integra a configuração versão 1. Configurações anteriores
sem esse campo carregam como lista vazia. O backend valida até 64 dispositivos,
IDs únicos e um único cadastro por par tipo/interface. Uma interface composta pode
ser associada uma vez a mouse e outra a teclado. Nome e propriedades podem ser
editados sem a interface conectada. Um vínculo novo exige interface compatível
presente no snapshot. Não há identificação automática do modelo físico.

Trocar a interface transfere as ações específicas do tipo de entrada correspondente
(mouse/teclado) em todos os perfis. Se o destino já contém uma ação para um desses
controles, a gravação é recusada com mensagem de conflito. Ações gerais e do outro
tipo de entrada são preservadas. A gravação usa a mesma revisão otimista e troca
atômica do arquivo de perfis. Reinicie daemon e shell Tauri após esta atualização,
pois binários antigos rejeitam campos desconhecidos da configuração.

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

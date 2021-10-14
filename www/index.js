import { Node as Gun } from "rusty-gun";

const gun = new Gun();

setTimeout(() => { // wait for websocket to open
  gun.get('latestMsg').on((v,k) => console.log(k,v));

  const profile = gun.get('profile');
  profile.get('name').get('first').on((name, key) => console.log('profile/name/first', name, key));
  profile.get('age').on(console.log);
  profile.get('isAwesome').on(console.log);

  profile.get('name').get('first').put('Martin');
  profile.get('age').put(32);
  profile.get('isAwesome').put(true);

  const onListenerId = profile.on((profile, key) => console.log('profile.on()', profile, key));
  const mapListenerId = profile.map((val, key) => console.log('profile.map()', val, key));

  console.log('onListenerId', onListenerId, 'mapListenerId', mapListenerId);

  setTimeout(() => {
    profile.get('age').put(33);
  }, 1000);
}, 1000);

window.gun = gun;
window.Gun = Gun;